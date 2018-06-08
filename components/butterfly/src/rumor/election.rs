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

//! Leader election.
//!
//! This module does leader election for services. It consists of an `Election` that implements
//! `Rumor`, and uses a variant of the [Bully
//! Algorithm](https://en.wikipedia.org/wiki/Bully_algorithm) to select the leader.
//!
//! It uses a particular variant I think of as the "highlander" model. A given election will
//! devolve to a single, universal rumor, which when it is received by the winner will result in
//! the election finishing. There can, in the end, be only one.

use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use habitat_core::service::ServiceGroup;
use prost::Message;

use error::{Error, Result};
use protocol::newscast::Rumor as ProtoRumor;
pub use protocol::newscast::{election::Status as ElectionStatus, Election as ProtoElection};
use protocol::{self, FromProto};
use rumor::{Rumor, RumorPayload, RumorType};

#[derive(Debug, Clone, Serialize)]
pub struct Election {
    pub from_id: String,
    pub member_id: String,
    pub service_group: ServiceGroup,
    pub term: u64,
    pub suitability: u64,
    pub status: ElectionStatus,
    pub votes: Vec<String>,
}

impl Election {
    /// Create a new election, voting for the given member id, for the given service group, and
    /// with the given suitability.
    pub fn new<S1>(member_id: S1, service_group: ServiceGroup, suitability: u64) -> Election
    where
        S1: Into<String>,
    {
        let from_id = member_id.into();
        Election {
            from_id: from_id.clone(),
            member_id: from_id,
            service_group: service_group,
            term: 0,
            suitability: suitability,
            status: ElectionStatus::Running,
            votes: vec![from_id],
        }
    }

    /// Insert a vote for the election.
    pub fn insert_vote(&mut self, member_id: &str) {
        if !self.votes.contains(&String::from(member_id)) {
            self.votes.push(String::from(member_id));
        }
    }

    /// Steal all the votes from another election for ourselves.
    pub fn steal_votes(&mut self, other: &mut Election) {
        for x in other.votes.iter() {
            self.insert_vote(x);
        }
    }

    /// Sets the status of the election to "running".
    pub fn running(&mut self) {
        self.status = ElectionStatus::Running;
    }

    /// Sets the status of the election to "finished"
    pub fn finish(&mut self) {
        self.status = ElectionStatus::Finished;
    }

    /// Sets the status of the election to "NoQuorum"
    pub fn no_quorum(&mut self) {
        self.status = ElectionStatus::NoQuorum;
    }

    /// Returns true if the election is finished.
    pub fn is_finished(&self) -> bool {
        self.status == ElectionStatus::Finished
    }
}

impl PartialEq for Election {
    /// We ignore id in equality checking, because we only have one per service group
    fn eq(&self, other: &Election) -> bool {
        self.service_group == other.service_group && self.member_id == other.member_id
            && self.suitability == other.suitability && self.votes == other.votes
            && self.status == other.status && self.term == other.term
    }
}

impl protocol::Message<ProtoRumor> for Election {}

impl FromProto<ProtoRumor> for Election {
    fn from_proto(rumor: ProtoRumor) -> Result<Self> {
        let payload = match rumor.payload.ok_or(Error::ProtocolMismatch("payload"))? {
            RumorPayload::Election(payload) => payload,
            _ => panic!("from-bytes election"),
        };
        let from_id = rumor.from_id.ok_or(Error::ProtocolMismatch("from-id"))?;
        Ok(Election {
            from_id: from_id.clone(),
            member_id: from_id.clone(),
            service_group: payload
                .service_group
                .ok_or(Error::ProtocolMismatch("service-group"))
                .and_then(|s| ServiceGroup::from_str(&s).map_err(Error::from))?,
            term: payload.term.unwrap_or(0),
            suitability: payload.suitability.unwrap_or(0),
            status: payload
                .status
                .and_then(ElectionStatus::from_i32)
                .unwrap_or(ElectionStatus::Running),
            votes: payload.votes,
        })
    }
}

impl Rumor for Election {
    /// Updates this election based on the contents of another election.
    fn merge(&mut self, mut other: Election) -> bool {
        if *self == other {
            // If we are the same object, just return false
            false
        } else if other.term >= self.term && other.status == ElectionStatus::Finished {
            // If the new rumors term is bigger or equal to ours, and it has a leader, we take it as
            // the leader and move on.
            *self = other;
            true
        } else if other.term == self.term && self.status == ElectionStatus::Finished {
            // If the terms are equal, and we are finished, then we drop the other side on the
            // floor
            false
        } else if self.term > other.term {
            // If the rumor we got has a term that's lower than ours, keep sharing our rumor no
            // matter what term they are on.
            true
        } else if self.suitability > other.suitability {
            // If we are more suitable than the other side, we want to steal
            // the other sides votes, and keep sharing.
            self.steal_votes(&mut other);
            true
        } else if other.suitability > self.suitability {
            // If the other side is more suitable than we are, we want to add our votes
            // to its tally, then take it as our rumor.
            other.steal_votes(self);
            *self = other;
            true
        } else {
            if self.member_id >= other.member_id {
                // If we are equally suitable, and our id sorts before the other, we want to steal
                // it's votes, and mark it as having voted for us.
                self.steal_votes(&mut other);
                true
            } else {
                // If we are equally suitable, but the other id sorts before ours, then we give it
                // our votes, vote for it ourselves, and spread it as the new rumor
                other.steal_votes(self);
                *self = other;
                true
            }
        }
    }

    /// We are the Election rumor!
    fn kind(&self) -> RumorType {
        RumorType::Election
    }

    /// There can be only
    fn id(&self) -> &str {
        "election"
    }

    fn key(&self) -> &str {
        self.service_group.as_ref()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ElectionUpdate(Election);

impl ElectionUpdate {
    pub fn new<S1>(member_id: S1, service_group: ServiceGroup, suitability: u64) -> ElectionUpdate
    where
        S1: Into<String>,
    {
        let election = Election::new(member_id, service_group, suitability);
        ElectionUpdate(election)
    }
}

impl Deref for ElectionUpdate {
    type Target = Election;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ElectionUpdate {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Election> for ElectionUpdate {
    fn from(other: Election) -> Self {
        ElectionUpdate(other)
    }
}

impl protocol::Message<ProtoRumor> for ElectionUpdate {}

impl FromProto<ProtoRumor> for ElectionUpdate {
    fn from_proto(rumor: ProtoRumor) -> Result<Self> {
        Ok(ElectionUpdate(Election::from_proto(rumor)?))
    }
}

impl Rumor for ElectionUpdate {
    fn merge(&mut self, other: ElectionUpdate) -> bool {
        self.0.merge(other.0)
    }

    fn kind(&self) -> RumorType {
        RumorType::ElectionUpdate
    }

    fn id(&self) -> &str {
        "election"
    }

    fn key(&self) -> &str {
        self.0.key()
    }
}

#[cfg(test)]
mod tests {
    use habitat_core::service::ServiceGroup;
    use rumor::election::Election;
    use rumor::Rumor;

    fn create_election(member_id: &str, suitability: u64) -> Election {
        Election::new(
            member_id,
            ServiceGroup::new(None, "tdep", "prod", None).unwrap(),
            suitability,
        )
    }

    #[test]
    fn merge_two_identical_elections_returns_false() {
        let mut e1 = create_election("a", 0);
        let e2 = e1.clone();
        assert_eq!(e1.merge(e2), false);
    }

    #[test]
    fn merge_four_one_higher_suitability() {
        let mut e1 = create_election("a", 0);
        let e2 = create_election("b", 0);
        let e3 = create_election("c", 1);
        let e4 = create_election("d", 0);
        assert_eq!(e1.merge(e2), true);
        assert_eq!(e1.merge(e3), true);
        assert_eq!(e1.merge(e4), true);
        assert_eq!(e1.member_id, "c");
        assert_eq!(e1.votes.len(), 4);
    }

    #[test]
    fn merge_four() {
        let mut e1 = create_election("a", 0);
        let e2 = create_election("b", 0);
        let e3 = create_election("c", 0);
        let e4 = create_election("d", 0);
        assert_eq!(e1.merge(e2), true);
        assert_eq!(e1.merge(e3), true);
        assert_eq!(e1.merge(e4), true);
        assert_eq!(e1.member_id, "d");
        assert_eq!(e1.votes.len(), 4);
    }
}
