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

//! The Departure rumor.
//!
//! Deaprture rumors declare that a given member has "departed" the gossip ring manually. When this
//! happens, we ensure that the member can no longer come back into the fold, unless an
//! administrator reverses the decision.

use std::cmp::Ordering;

use bytes::BytesMut;
use prost::Message;

use error::{Error, Result};
use protocol::{self,
               swim::{Departure as ProtoDeparture, Rumor as ProtoRumor}};
use rumor::{Rumor, RumorPayload, RumorType};

#[derive(Debug, Clone, Serialize)]
pub struct Departure {
    pub member_id: String,
}

impl Departure {
    pub fn new<U>(member_id: U) -> Self
    where
        U: ToString,
    {
        Departure {
            member_id: member_id.to_string(),
        }
    }
}

impl protocol::Message for Departure {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let rumor = ProtoRumor::decode(bytes)?;
        let payload = match rumor.payload.ok_or(Error::ProtocolMismatch("payload"))? {
            RumorPayload::Departure(payload) => payload,
            _ => panic!("from-bytes departure"),
        };
        Ok(Departure {
            member_id: rumor.member_id.ok_or(Error::ProtocolMismatch("member-id"))?,
        })
    }

    fn write_to_bytes(&self) -> Result<Vec<u8>> {
        let payload = ProtoDeparture {
            member_id: Some(self.member_id),
        };
        let rumor = ProtoRumor {
            type_: self.kind() as i32,
            tag: Vec::default(),
            from_id: "butterflyclient".to_string(),
            payload: Some(RumorPayload::Departure(payload)),
        };
        let mut buf = BytesMut::with_capacity(rumor.encoded_len());
        rumor.encode(&mut buf)?;
        Ok(buf.to_vec())
    }
}

impl Rumor for Departure {
    fn merge(&mut self, other: Departure) -> bool {
        if *self >= other {
            false
        } else {
            true
        }
    }

    fn kind(&self) -> RumorType {
        RumorType::Departure
    }

    fn id(&self) -> &str {
        &self.member_id
    }

    fn key(&self) -> &str {
        "departure"
    }
}

impl PartialOrd for Departure {
    fn partial_cmp(&self, other: &Departure) -> Option<Ordering> {
        if self.member_id != other.member_id {
            None
        } else {
            Some(self.member_id.cmp(&other.member_id))
        }
    }
}

impl PartialEq for Departure {
    fn eq(&self, other: &Departure) -> bool {
        self.member_id == other.member_id
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::Departure;
    use rumor::Rumor;

    fn create_departure(member_id: &str) -> Departure {
        Departure::new(member_id)
    }

    #[test]
    fn identical_departures_are_equal() {
        let s1 = create_departure("mastodon");
        let s2 = create_departure("mastodon");
        assert_eq!(s1, s2);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn departures_with_different_member_ids_are_not_equal() {
        let s1 = create_departure("mastodon");
        let s2 = create_departure("limpbizkit");
        assert_eq!(s1, s2);
    }

    // Order
    #[test]
    fn departures_that_are_identical_are_equal_via_cmp() {
        let s1 = create_departure("adam");
        let s2 = create_departure("adam");
        assert_eq!(s1.partial_cmp(&s2), Some(Ordering::Equal));
    }

    #[test]
    fn merge_returns_false_if_nothing_changed() {
        let mut s1 = create_departure("mastodon");
        let s1_check = s1.clone();
        let s2 = create_departure("mastodon");
        assert_eq!(s1.merge(s2), false);
        assert_eq!(s1, s1_check);
    }
}
