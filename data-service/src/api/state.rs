use std::sync::Arc;

use crate::domain::{
    group::GroupRepository, membership::MembershipRepository, staff::StaffRepository,
};

pub struct DataServiceAppState {
    pub staff_repo: Arc<dyn StaffRepository>,
    pub group_repo: Arc<dyn GroupRepository>,
    pub membership_repo: Arc<dyn MembershipRepository>,
}
