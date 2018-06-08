// Copyright (c) 2016-2017 Chef Software Inc. and/or applicable contributors
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

//! The ServiceConfig rumor.
//!
//! Holds the toml configuration injected for a service.

use std::cmp::Ordering;
use std::mem;
use std::str::{self, FromStr};

use habitat_core::crypto::{default_cache_key_path, BoxKeyPair};
use habitat_core::service::ServiceGroup;
use toml;

use error::{Error, Result};
use protocol::{self, newscast::Rumor as ProtoRumor, FromProto};
use rumor::{Rumor, RumorPayload, RumorType};

#[derive(Debug, Clone, Serialize)]
pub struct ServiceConfig {
    pub from_id: String,
    pub service_group: ServiceGroup,
    pub incarnation: u64,
    pub encrypted: bool,
    pub config: Vec<u8>,
}

impl PartialOrd for ServiceConfig {
    fn partial_cmp(&self, other: &ServiceConfig) -> Option<Ordering> {
        if self.service_group != other.service_group {
            None
        } else {
            Some(self.incarnation.cmp(&other.incarnation))
        }
    }
}

impl PartialEq for ServiceConfig {
    fn eq(&self, other: &ServiceConfig) -> bool {
        self.service_group == other.service_group && self.incarnation == other.incarnation
            && self.encrypted == other.encrypted && self.config == other.config
    }
}

impl ServiceConfig {
    /// Creates a new ServiceConfig.
    pub fn new<S1>(member_id: S1, service_group: ServiceGroup, config: Vec<u8>) -> Self
    where
        S1: Into<String>,
    {
        ServiceConfig {
            from_id: member_id.into(),
            service_group: service_group,
            incarnation: 0,
            encrypted: false,
            config: config,
        }
    }

    pub fn encrypt(&mut self, user_pair: &BoxKeyPair, service_pair: &BoxKeyPair) -> Result<()> {
        self.config = user_pair.encrypt(&self.config, Some(service_pair))?;
        self.encrypted = true;
        Ok(())
    }

    pub fn config(&self) -> Result<toml::value::Table> {
        let config = if self.encrypted {
            let bytes = BoxKeyPair::decrypt_with_path(&self.config, &default_cache_key_path(None))?;
            let encoded = str::from_utf8(&bytes)
                .map_err(|e| Error::ServiceConfigNotUtf8(self.service_group.to_string(), e))?;
            self.parse_config(&encoded)?
        } else {
            let encoded = str::from_utf8(&self.config)
                .map_err(|e| Error::ServiceConfigNotUtf8(self.service_group.to_string(), e))?;
            self.parse_config(&encoded)?
        };
        Ok(config)
    }

    fn parse_config(&self, encoded: &str) -> Result<toml::value::Table> {
        toml::from_str(encoded)
            .map_err(|e| Error::ServiceConfigDecode(self.service_group.to_string(), e))
    }
}

impl protocol::Message<ProtoRumor> for ServiceConfig {}

impl FromProto<ProtoRumor> for ServiceConfig {
    fn from_proto(rumor: ProtoRumor) -> Result<Self> {
        let payload = match rumor.payload.ok_or(Error::ProtocolMismatch("payload"))? {
            RumorPayload::ServiceConfig(payload) => payload,
            _ => panic!("from-bytes service-config"),
        };
        Ok(ServiceConfig {
            from_id: rumor.from_id.ok_or(Error::ProtocolMismatch("from-id"))?,
            service_group: payload
                .service_group
                .ok_or(Error::ProtocolMismatch("service-group"))
                .and_then(|s| ServiceGroup::from_str(&s).map_err(Error::from))?,
            incarnation: payload.incarnation.unwrap_or(0),
            encrypted: payload.encrypted.unwrap_or(false),
            config: payload.config.unwrap_or_default(),
        })
    }
}

impl Rumor for ServiceConfig {
    /// Follows a simple pattern; if we have a newer incarnation than the one we already have, the
    /// new one wins. So far, these never change.
    fn merge(&mut self, mut other: ServiceConfig) -> bool {
        if *self >= other {
            false
        } else {
            mem::swap(self, &mut other);
            true
        }
    }

    fn kind(&self) -> RumorType {
        RumorType::ServiceConfig
    }

    fn id(&self) -> &str {
        "service_config"
    }

    fn key(&self) -> &str {
        &self.service_group
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use habitat_core::service::ServiceGroup;
    use toml;

    use super::ServiceConfig;
    use rumor::Rumor;

    fn create_service_config(member_id: &str, config: &str) -> ServiceConfig {
        let config_bytes: Vec<u8> = Vec::from(config);
        ServiceConfig::new(
            member_id,
            ServiceGroup::new(None, "neurosis", "production", None).unwrap(),
            config_bytes,
        )
    }

    #[test]
    fn identical_service_config_are_equal() {
        let s1 = create_service_config("adam", "yep");
        let s2 = create_service_config("adam", "yep");
        assert_eq!(s1, s2);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn service_configs_with_different_incarnations_are_not_equal() {
        let s1 = create_service_config("adam", "yep");
        let mut s2 = create_service_config("adam", "yep");
        s2.incarnation = 1;
        assert_eq!(s1, s2);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn service_configs_with_different_service_groups_are_not_equal() {
        let s1 = create_service_config("adam", "yep");
        let mut s2 = create_service_config("adam", "yep");
        s2.service_group = String::from("adam.fragile");
        assert_eq!(s1, s2);
    }

    // Order
    #[test]
    fn service_configs_that_are_identical_are_equal_via_cmp() {
        let s1 = create_service_config("adam", "yep");
        let s2 = create_service_config("adam", "yep");
        assert_eq!(s1.partial_cmp(&s2), Some(Ordering::Equal));
    }

    #[test]
    fn service_configs_with_different_incarnations_are_not_equal_via_cmp() {
        let s1 = create_service_config("adam", "yep");
        let mut s2 = create_service_config("adam", "yep");
        s2.incarnation = 1;
        assert_eq!(s1.partial_cmp(&s2), Some(Ordering::Less));
        assert_eq!(s2.partial_cmp(&s1), Some(Ordering::Greater));
    }

    #[test]
    fn merge_chooses_the_higher_incarnation() {
        let mut s1 = create_service_config("adam", "yep");
        let mut s2 = create_service_config("adam", "yep");
        s2.incarnation = 1;
        let s2_check = s2.clone();
        assert_eq!(s1.merge(s2), true);
        assert_eq!(s1, s2_check);
    }

    #[test]
    fn merge_returns_false_if_nothing_changed() {
        let mut s1 = create_service_config("adam", "yep");
        s1.incarnation = 1;
        let s1_check = s1.clone();
        let s2 = create_service_config("adam", "yep");
        assert_eq!(s1.merge(s2), false);
        assert_eq!(s1, s1_check);
    }

    #[test]
    fn config_comes_back_as_a_toml_value() {
        let s1 = create_service_config("adam", "yep=1");
        assert_eq!(
            s1.config().unwrap(),
            toml::from_str::<toml::value::Table>("yep=1").unwrap()
        );
    }
}
