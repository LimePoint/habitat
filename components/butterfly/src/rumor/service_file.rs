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

//! The ServiceFile rumor.
//!
//! Holds the toml configuration injected for a service.

use std::cmp::Ordering;
use std::mem;

use bytes::BytesMut;
use habitat_core::crypto::{default_cache_key_path, BoxKeyPair};
use habitat_core::service::ServiceGroup;
use prost::Message;

use error::{Error, Result};
use protocol::{self,
               swim::{Rumor as ProtoRumor, ServiceFile as ProtoServiceFile}};
use rumor::{Rumor, RumorPayload, RumorType};

#[derive(Debug, Clone, Serialize)]
pub struct ServiceFile {
    pub from_id: String,
    pub service_group: ServiceGroup,
    pub incarnation: u64,
    pub encrypted: bool,
    pub filename: String,
    pub body: Vec<u8>,
}

impl PartialOrd for ServiceFile {
    fn partial_cmp(&self, other: &ServiceFile) -> Option<Ordering> {
        if self.service_group != other.service_group {
            None
        } else {
            Some(self.incarnation.cmp(&other.incarnation))
        }
    }
}

impl PartialEq for ServiceFile {
    fn eq(&self, other: &ServiceFile) -> bool {
        self.service_group == other.service_group && self.incarnation == other.incarnation
            && self.encrypted == other.encrypted && self.filename == other.filename
            && self.body == other.body
    }
}

impl ServiceFile {
    /// Creates a new ServiceFile.
    pub fn new<S1, S2>(
        member_id: S1,
        service_group: ServiceGroup,
        filename: S2,
        body: Vec<u8>,
    ) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        ServiceFile {
            from_id: member_id.into(),
            service_group: service_group,
            incarnation: 0,
            encrypted: false,
            filename: filename.into(),
            body: body,
        }
    }

    /// Encrypt the contents of the service file
    pub fn encrypt(&mut self, user_pair: &BoxKeyPair, service_pair: &BoxKeyPair) -> Result<()> {
        self.body = user_pair.encrypt(&self.body, Some(service_pair))?;
        self.encrypted = true;
        Ok(())
    }

    /// Return the body of the service file as a stream of bytes. Always returns a new copy, due to
    /// the fact that we might be encrypted.
    pub fn body(&self) -> Result<Vec<u8>> {
        if self.encrypted {
            let bytes = BoxKeyPair::decrypt_with_path(&self.body, &default_cache_key_path(None))?;
            Ok(bytes)
        } else {
            Ok(self.body.to_vec())
        }
    }
}

impl protocol::Message for ServiceFile {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let rumor = ProtoRumor::decode(bytes)?;
        let payload = match rumor.payload.ok_or(Error::ProtocolMismatch("payload"))? {
            RumorPayload::ServiceFile(payload) => payload,
            _ => panic!("from-bytes service-config"),
        };
        Ok(ServiceFile {
            from_id: rumor.from_id.ok_or(Error::ProtocolMismatch("from-id"))?,
            service_group: rumor
                .service_group
                .ok_or(Error::ProtocolMismatch("service-group"))?
                .and_then(ServiceGroup::from_str)?,
            incarnation: rumor.incarnation.unwrap_or(0),
            encrypted: rumor.initialized.unwrap_or(false),
            filename: rumor.filename.ok_or(Error::ProtocolMismatch("filename"))?,
            body: rumor.body.unwrap_or_default(),
        })
    }

    fn write_to_bytes(&self) -> Result<Vec<u8>> {
        let payload = ProtoServiceFile {
            service_group: Some(self.service_group.to_string()),
            incarnation: Some(self.incarnation),
            encrypted: Some(self.encrypted),
            filename: Some(self.filename),
            body: Some(self.body),
        };
        let rumor = ProtoRumor {
            type_: self.kind() as i32,
            tag: Vec::default(),
            from_id: self.member_id.clone(),
            payload: Some(RumorPayload::ServiceFile(payload)),
        };
        let mut buf = BytesMut::with_capacity(rumor.encoded_len());
        rumor.encode(&mut buf)?;
        Ok(buf.to_vec())
    }
}

impl Rumor for ServiceFile {
    /// Follows a simple pattern; if we have a newer incarnation than the one we already have, the
    /// new one wins. So far, these never change.
    fn merge(&mut self, mut other: ServiceFile) -> bool {
        if *self >= other {
            false
        } else {
            mem::swap(self, &mut other);
            true
        }
    }

    fn kind(&self) -> RumorType {
        RumorType::ServiceFile
    }

    fn id(&self) -> &str {
        &self.filename
    }

    fn key(&self) -> &str {
        &self.service_group
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use habitat_core::service::ServiceGroup;

    use super::ServiceFile;
    use rumor::Rumor;

    fn create_service_file(member_id: &str, filename: &str, body: &str) -> ServiceFile {
        let body_bytes: Vec<u8> = Vec::from(body);
        ServiceFile::new(
            member_id,
            ServiceGroup::new(None, "neurosis", "production", None).unwrap(),
            filename,
            body_bytes,
        )
    }

    #[test]
    fn identical_service_file_are_equal() {
        let s1 = create_service_file("adam", "yep", "tcp-backlog = 128");
        let s2 = create_service_file("adam", "yep", "tcp-backlog = 128");
        assert_eq!(s1, s2);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn service_files_with_different_incarnations_are_not_equal() {
        let s1 = create_service_file("adam", "yep", "tcp-backlog = 128");
        let mut s2 = create_service_file("adam", "yep", "tcp-backlog = 128");
        s2.set_incarnation(1);
        assert_eq!(s1, s2);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn service_files_with_different_service_groups_are_not_equal() {
        let s1 = create_service_file("adam", "yep", "tcp-backlog = 128");
        let mut s2 = create_service_file("adam", "yep", "tcp-backlog = 128");
        s2.set_service_group(String::from("adam.fragile"));
        assert_eq!(s1, s2);
    }

    // Order
    #[test]
    fn service_files_that_are_identical_are_equal_via_cmp() {
        let s1 = create_service_file("adam", "yep", "tcp-backlog = 128");
        let s2 = create_service_file("adam", "yep", "tcp-backlog = 128");
        assert_eq!(s1.partial_cmp(&s2), Some(Ordering::Equal));
    }

    #[test]
    fn service_files_with_different_incarnations_are_not_equal_via_cmp() {
        let s1 = create_service_file("adam", "yep", "tcp-backlog = 128");
        let mut s2 = create_service_file("adam", "yep", "tcp-backlog = 128");
        s2.set_incarnation(1);
        assert_eq!(s1.partial_cmp(&s2), Some(Ordering::Less));
        assert_eq!(s2.partial_cmp(&s1), Some(Ordering::Greater));
    }

    #[test]
    fn merge_chooses_the_higher_incarnation() {
        let mut s1 = create_service_file("adam", "yep", "tcp-backlog = 128");
        let mut s2 = create_service_file("adam", "yep", "tcp-backlog = 128");
        s2.set_incarnation(1);
        let s2_check = s2.clone();
        assert_eq!(s1.merge(s2), true);
        assert_eq!(s1, s2_check);
    }

    #[test]
    fn merge_returns_false_if_nothing_changed() {
        let mut s1 = create_service_file("adam", "yep", "tcp-backlog = 128");
        s1.set_incarnation(1);
        let s1_check = s1.clone();
        let s2 = create_service_file("adam", "yep", "tcp-backlog = 128");
        assert_eq!(s1.merge(s2), false);
        assert_eq!(s1, s1_check);
    }

    #[test]
    fn config_comes_back_as_a_string() {
        let s1 = create_service_file("adam", "yep", "tcp-backlog = 128");
        assert_eq!(
            String::from_utf8(s1.body().unwrap()).expect("cannot get a utf-8 string for the body"),
            String::from("tcp-backlog = 128")
        );
    }
}
