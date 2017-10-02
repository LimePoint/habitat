// Copyright (c) 2017 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::str::FromStr;

use bodyparser;
use bldr_core::build_config::{BLDR_CFG, BuildCfg};
use github_api_client::GitHubClient;
use hab_core::package::Plan;
use http_gateway::http::controller::*;
use iron::status;
use persistent;
use protocol::scheduler::{Group, GroupCreate};

use error::Error;
use headers::*;
use types::*;

pub enum GitHubEvent {
    Push,
    Ping,
}

impl FromStr for GitHubEvent {
    type Err = Error;

    fn from_str(event: &str) -> Result<Self, Self::Err> {
        match event {
            "ping" => Ok(GitHubEvent::Ping),
            "push" => Ok(GitHubEvent::Push),
            _ => Err(Error::UnknownGitHubEvent(event.to_string())),
        }
    }
}

enum HandleResult<T> {
    Ok(T),
    Err(Response),
}

pub fn handle_event(req: &mut Request) -> IronResult<Response> {
    let event = match req.headers.get::<XGitHubEvent>() {
        Some(&XGitHubEvent(ref event)) => {
            match GitHubEvent::from_str(event) {
                Ok(event) => event,
                Err(err) => return Ok(Response::with((status::BadRequest, err.to_string()))),
            }
        }
        _ => return Ok(Response::with(status::BadRequest)),
    };
    match event {
        GitHubEvent::Ping => Ok(Response::with(status::Ok)),
        GitHubEvent::Push => handle_push(req),
    }
}

fn handle_push(req: &mut Request) -> IronResult<Response> {
    // JW TODO: THIS NEEDS TO BE AUTHENTICATED
    // use sender.login to get user ID to get account & see if they have access to origin
    let hook = match req.get::<bodyparser::Struct<GitHubWebhookPush>>() {
        Ok(Some(hook)) => hook,
        Ok(None) => return Ok(Response::with(status::UnprocessableEntity)),
        Err(err) => {
            return Ok(Response::with(
                (status::UnprocessableEntity, err.to_string()),
            ));
        }
    };
    let github = req.get::<persistent::Read<GitHubCli>>().unwrap();
    let token = match github.app_installation_token(hook.installation.id) {
        Ok(token) => token,
        Err(err) => {
            return Ok(Response::with((status::BadGateway, err.to_string())));
        }
    };
    // JW TODO: Add searching for Windows plans (ps1) when Windows builders are completed
    let plans = match github.search_file(&token, &hook.repository.full_name, "plan.sh") {
        Ok(search) => search.items,
        Err(err) => return Ok(Response::with((status::BadGateway, err.to_string()))),
    };
    if plans.is_empty() {
        return Ok(Response::with(status::Ok));
    }
    let config = match github.search_file(&token, &hook.repository.full_name, &BLDR_CFG) {
        Ok(search) => {
            match search
                .items
                .into_iter()
                .filter(|i| i.path == BLDR_CFG)
                .collect::<Vec<SearchItem>>()
                .pop() {
                Some(item) => {
                    match read_bldr_config(&*github, &token, &hook, &item.path) {
                        HandleResult::Ok(cfg) => Some(cfg),
                        HandleResult::Err(response) => return Ok(response),
                    }
                }
                None => None,
            }
        }
        Err(err) => return Ok(Response::with((status::BadGateway, err.to_string()))),
    };
    debug!("Config, {:?}", config);
    let mut plans = match read_plans(&github, &token, &hook, plans) {
        HandleResult::Ok(plans) => plans,
        HandleResult::Err(err) => return Ok(err),
    };
    debug!("Plans, {:?}", plans);
    if let Some(cfg) = config {
        plans.retain(|plan| match cfg.get(&plan.name) {
            Some(project) => hook.changed().iter().any(|f| project.triggered_by(f)),
            None => false,
        })
    }
    build_plans(req, plans)
}

fn build_plans(req: &mut Request, plans: Vec<Plan>) -> IronResult<Response> {
    // JW TODO: Validate that this repository is where these plans belong. You could theoretically
    // create a plan in a different repo and force a build of another piece of software without
    // this check.
    let mut request = GroupCreate::new();
    for plan in plans.into_iter() {
        debug!("Scheduling, {:?}", plan);
        request.set_origin(plan.origin);
        request.set_package(plan.name);
        // JW TODO: We need to be able to determine which platform this build is for based on
        // the directory structure the plan is found in or metadata inside the plan. We will need
        // to have this done before we support building additional targets with Builder.
        request.set_target("x86_64-linux".to_string());
        match route_message::<GroupCreate, Group>(req, &request) {
            Ok(group) => debug!("Group created, {:?}", group),
            Err(err) => debug!("Failed to create group, {:?}", err),
        }
    }
    Ok(Response::with(status::Ok))
}

fn read_bldr_config(
    github: &GitHubClient,
    token: &str,
    hook: &GitHubWebhookPush,
    path: &str,
) -> HandleResult<BuildCfg> {
    match github.contents(
        token,
        &hook.repository.organization,
        &hook.repository.name,
        path,
    ) {
        Ok(contents) => {
            match contents.decode() {
                Ok(ref bytes) => {
                    match BuildCfg::from_slice(bytes) {
                        Ok(cfg) => HandleResult::Ok(cfg),
                        Err(err) => HandleResult::Err(Response::with(
                            (status::UnprocessableEntity, err.to_string()),
                        )),
                    }
                }
                Err(err) => {
                    HandleResult::Err(Response::with(
                        (status::UnprocessableEntity, err.to_string()),
                    ))
                }
            }
        }
        Err(err) => HandleResult::Err(Response::with((status::BadGateway, err.to_string()))),
    }
}

fn read_plans(
    github: &GitHubClient,
    token: &str,
    hook: &GitHubWebhookPush,
    plans: Vec<SearchItem>,
) -> HandleResult<Vec<Plan>> {
    let mut parsed = Vec::with_capacity(plans.len());
    for plan in plans {
        match github.contents(
            token,
            &hook.repository.organization,
            &hook.repository.name,
            &plan.path,
        ) {
            Ok(contents) => {
                match contents.decode() {
                    Ok(bytes) => {
                        match Plan::from_bytes(bytes.as_slice()) {
                            Ok(plan) => parsed.push(plan),
                            Err(err) => {
                                return HandleResult::Err(Response::with(
                                    (status::UnprocessableEntity, err.to_string()),
                                ))
                            }
                        }
                    }
                    Err(err) => {
                        return HandleResult::Err(Response::with(
                            (status::UnprocessableEntity, err.to_string()),
                        ))
                    }
                }
            }
            Err(err) => {
                return HandleResult::Err(Response::with((status::BadGateway, err.to_string())))
            }
        }
    }
    HandleResult::Ok(parsed)
}