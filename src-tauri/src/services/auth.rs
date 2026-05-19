use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    SchoolLeader,
    Teacher,
    Viewer,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PermissionSet {
    pub role: Role,
    pub permissions: Vec<String>,
}

pub fn permissions_for(role: &Role) -> Vec<String> {
    match role {
        Role::Admin => vec![
            "manage_users",
            "manage_school",
            "edit_curriculum",
            "publish_plan",
            "record_attendance",
            "generate_reports",
            "export_private_data",
        ],
        Role::SchoolLeader => vec![
            "manage_school",
            "edit_curriculum",
            "publish_plan",
            "record_attendance",
            "generate_reports",
            "export_standard_data",
        ],
        Role::Teacher => vec![
            "edit_curriculum",
            "record_attendance",
            "generate_reports",
            "export_standard_data",
        ],
        Role::Viewer => vec!["view_school", "view_reports"],
    }
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub fn can(role: &Role, action: &str) -> bool {
    permissions_for(role)
        .iter()
        .any(|permission| permission == action)
}

#[tauri::command]
pub async fn get_role_permissions(role: Role) -> Result<PermissionSet, String> {
    Ok(PermissionSet {
        permissions: permissions_for(&role),
        role,
    })
}

#[tauri::command]
pub async fn can_perform_action(role: Role, action: String) -> Result<bool, String> {
    Ok(can(&role, &action))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teacher_cannot_export_private_data() {
        assert!(can(&Role::Teacher, "record_attendance"));
        assert!(!can(&Role::Teacher, "export_private_data"));
    }
}
