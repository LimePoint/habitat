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

//! The Service rumor.
//!
//! Service rumors declare that a given `Server` is running this Service.

use std::cmp::Ordering;
use std::mem;
use std::str::FromStr;

use bytes::BytesMut;
use habitat_core::package::Identifiable;
use habitat_core::service::ServiceGroup;
use prost::Message;
use toml;

use error::{Error, Result};
pub use protocol::swim::SysInfo;
use protocol::{self,
               swim::{Rumor as ProtoRumor, Service as ProtoService}};
use rumor::{Rumor, RumorPayload, RumorType};

#[derive(Debug, Clone, Serialize)]
pub struct Service {
    pub member_id: String,
    pub service_group: ServiceGroup,
    pub incarnation: u64,
    pub initialized: bool,
    pub pkg: String,
    pub cfg: Vec<u8>,
    pub sys: SysInfo,
}

impl PartialOrd for Service {
    fn partial_cmp(&self, other: &Service) -> Option<Ordering> {
        if self.member_id != other.member_id || self.service_group != other.service_group {
            None
        } else {
            Some(self.incarnation.cmp(&other.incarnation))
        }
    }
}

impl PartialEq for Service {
    fn eq(&self, other: &Service) -> bool {
        self.member_id == other.member_id && self.service_group == other.service_group
            && self.incarnation == other.incarnation
    }
}

impl Service {
    /// Creates a new Service.
    pub fn new<T, U>(
        member_id: U,
        package: &T,
        service_group: ServiceGroup,
        sys: SysInfo,
        cfg: Option<&toml::value::Table>,
    ) -> Self
    where
        T: Identifiable,
        U: Into<String>,
    {
        assert!(
            package.fully_qualified(),
            "Service constructor requires a fully qualified package identifier"
        );
        assert_eq!(
            service_group.service(),
            package.name(),
            "Service constructor requires the given package name to match the service \
             group's name"
        );
        Service {
            member_id: member_id.into(),
            service_group: service_group,
            incarnation: 0,
            initialized: false,
            pkg: package.to_string(),
            sys: sys,
            // TODO FN: Can we really expect this all the time, should we return a `Result<Self>`
            // in this constructor?
            cfg: cfg.map(|v| toml::ser::to_vec(v).expect("Struct should serialize to bytes"))
                .unwrap_or_default(),
        }
    }
}

impl protocol::Message for Service {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let rumor = ProtoRumor::decode(bytes)?;
        let payload = match rumor.payload.ok_or(Error::ProtocolMismatch("payload"))? {
            RumorPayload::Service(payload) => payload,
            _ => panic!("from-bytes service"),
        };
        Ok(Service {
            member_id: rumor.member_id.ok_or(Error::ProtocolMismatch("member-id"))?,
            service_group: rumor
                .service_group
                .ok_or(Error::ProtocolMismatch("service-group"))?
                .and_then(ServiceGroup::from_str)?,
            incarnation: rumor.incarnation.unwrap_or(0),
            initialized: rumor.initialized.unwrap_or(false),
            pkg: rumor.pkg.ok_or(Error::ProtocolMismatch("pkg"))?,
            cfg: rumor.cfg.unwrap_or_default(),
            sys: rumor.sys.ok_or(Error::ProtocolMismatch("sys"))?,
        })
    }

    fn write_to_bytes(&self) -> Result<Vec<u8>> {
        let payload = ProtoService {
            member_id: Some(self.member_id),
            service_group: Some(self.service_group.to_string()),
            incarnation: Some(self.incarnation),
            initialized: Some(self.initialized),
            pkg: Some(self.pkg),
            cfg: Some(self.cfg),
            sys: Some(self.sys),
        };
        let rumor = ProtoRumor {
            type_: self.kind() as i32,
            tag: Vec::default(),
            from_id: self.member_id.clone(),
            payload: Some(RumorPayload::Service(payload)),
        };
        let mut buf = BytesMut::with_capacity(rumor.encoded_len());
        rumor.encode(&mut buf)?;
        Ok(buf.to_vec())
    }
}

impl Rumor for Service {
    /// Follows a simple pattern; if we have a newer incarnation than the one we already have, the
    /// new one wins. So far, these never change.
    fn merge(&mut self, mut other: Service) -> bool {
        if *self >= other {
            false
        } else {
            mem::swap(self, &mut other);
            true
        }
    }

    fn kind(&self) -> RumorType {
        RumorType::Service
    }

    fn id(&self) -> &str {
        &self.member_id
    }

    fn key(&self) -> &str {
        self.service_group.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::str::FromStr;

    use habitat_core::package::{Identifiable, PackageIdent};
    use habitat_core::service::ServiceGroup;

    use super::Service;
    use rumor::service::SysInfo;
    use rumor::Rumor;

    fn create_service(member_id: &str) -> Service {
        let pkg = PackageIdent::from_str("core/neurosis/1.2.3/20161208121212").unwrap();
        let sg = ServiceGroup::new(None, pkg.name(), "production", None).unwrap();
        Service::new(member_id.to_string(), &pkg, &sg, &SysInfo::default(), None)
    }

    #[test]
    fn identical_services_are_equal() {
        // Two different objects with the same member id, service group, and incarnation are equal
        let s1 = create_service("adam");
        let s2 = create_service("adam");
        assert_eq!(s1, s2);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn services_with_different_member_ids_are_not_equal() {
        let s1 = create_service("adam");
        let s2 = create_service("shanku");
        assert_eq!(s1, s2);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn services_with_different_incarnations_are_not_equal() {
        let s1 = create_service("adam");
        let mut s2 = create_service("adam");
        s2.set_incarnation(1);
        assert_eq!(s1, s2);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn services_with_different_service_groups_are_not_equal() {
        let s1 = create_service("adam");
        let mut s2 = create_service("adam");
        s2.set_service_group(String::from("adam.fragile"));
        assert_eq!(s1, s2);
    }

    // Order
    #[test]
    fn services_that_are_identical_are_equal_via_cmp() {
        let s1 = create_service("adam");
        let s2 = create_service("adam");
        assert_eq!(s1.partial_cmp(&s2), Some(Ordering::Equal));
    }

    #[test]
    fn services_with_different_incarnations_are_not_equal_via_cmp() {
        let s1 = create_service("adam");
        let mut s2 = create_service("adam");
        s2.set_incarnation(1);
        assert_eq!(s1.partial_cmp(&s2), Some(Ordering::Less));
        assert_eq!(s2.partial_cmp(&s1), Some(Ordering::Greater));
    }

    #[test]
    fn services_of_different_members_and_groups_cannot_be_compared() {
        let s1 = create_service("adam");
        let s2 = create_service("neurosis");
        assert_eq!(s1.partial_cmp(&s2), None);
    }

    #[test]
    fn merge_chooses_the_higher_incarnation() {
        let mut s1 = create_service("adam");
        let mut s2 = create_service("adam");
        s2.set_incarnation(1);
        let s2_check = s2.clone();
        assert_eq!(s1.merge(s2), true);
        assert_eq!(s1, s2_check);
    }

    #[test]
    fn merge_returns_false_if_nothing_changed() {
        let mut s1 = create_service("adam");
        s1.set_incarnation(1);
        let s1_check = s1.clone();
        let s2 = create_service("adam");
        assert_eq!(s1.merge(s2), false);
        assert_eq!(s1, s1_check);
    }

    #[test]
    #[should_panic]
    fn service_package_name_mismatch() {
        let ident = PackageIdent::from_str("core/overwatch/1.2.3/20161208121212").unwrap();
        let sg = ServiceGroup::new(None, "counter-strike", "times", Some("ofgrace")).unwrap();
        Service::new(
            "bad-member".to_string(),
            &ident,
            &sg,
            &SysInfo::default(),
            None,
        );
    }
}
