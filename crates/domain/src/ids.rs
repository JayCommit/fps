use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! entity_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            pub fn as_uuid(self) -> Uuid {
                self.0
            }

            pub fn as_str(&self) -> String {
                self.0.to_string()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                Display::fmt(&self.0, f)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }
    };
}

entity_id!(UserId);
entity_id!(SessionId);
entity_id!(NodeId);
entity_id!(EnrollmentTokenId);
entity_id!(AuditEventId);
entity_id!(InvitationId);
entity_id!(JobId);
entity_id!(ServerId);
entity_id!(RequestId);
entity_id!(TemplateId);
entity_id!(AllocationId);
entity_id!(BackupId);
entity_id!(ScheduleId);
entity_id!(NotificationId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_version_7() {
        let id = UserId::new();
        assert_eq!(id.0.get_version(), Some(uuid::Version::SortRand));
    }
}
