use utoipa::OpenApi;

use crate::api::{AddRequest, ApiBookmark, ErrorBody, ListResponse, UpdateRequest};

#[derive(OpenApi)]
#[openapi(
    info(title = "liber API", version = "2"),
    paths(
        crate::bookmarks::list_bookmarks,
        crate::bookmarks::add_bookmark,
        crate::bookmarks::get_bookmark,
        crate::bookmarks::update_bookmark,
        crate::bookmarks::delete_bookmark,
        crate::bookmarks::open_bookmark,
        crate::bookmarks::history,
        crate::bookmarks::download_attachment,
        crate::bookmarks::upload_attachment,
        crate::bookmarks::get_notes,
        crate::bookmarks::put_notes,
        crate::bookmarks::get_archive,
        crate::bookmarks::post_archive,
        crate::bookmarks::delete_archive,
        crate::bookmarks::delete_notes,
        crate::bookmarks::delete_attachment,
        crate::taxonomy::list_tags,
        crate::taxonomy::list_folders,
        crate::taxonomy::rename_tag_ep,
        crate::taxonomy::delete_tag_ep,
        crate::taxonomy::rename_folder_ep,
        crate::taxonomy::delete_folder_ep,
        crate::rules::list_rules,
        crate::rules::add_rule_ep,
        crate::rules::edit_rule_ep,
        crate::rules::delete_rule_ep,
        crate::rules::apply_rules_ep,
        crate::rules::learn_rules,
        crate::rules::learn_create,
        crate::check::run_check,
        crate::check::apply_check,
        crate::bulk::bulk,
        crate::library::import_library,
        crate::library::export_bookmarks,
        crate::library::export_site_ep,
        crate::sync::export_oplog,
        crate::sync::import_oplog,
        crate::sync::prune_oplog_ep,
        crate::sync::commit_snapshot,
        crate::sync::pick_bookmark,
        crate::profiles::list_profiles,
        crate::profiles::switch_profile,
        crate::profiles::delete_profile,
        crate::settings::get_settings,
        crate::settings::set_setting,
        crate::reindex::reindex_ep,
        crate::auth::login_page,
        crate::auth::login_submit,
        crate::auth::logout,
    ),
    components(schemas(ApiBookmark, AddRequest, UpdateRequest, ListResponse, ErrorBody,))
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("openapi.json")
    }

    #[test]
    fn openapi_snapshot_matches() {
        let doc = ApiDoc::openapi().to_pretty_json().unwrap() + "\n";
        if std::env::var("UPDATE_OPENAPI").is_ok() {
            std::fs::write(snapshot_path(), &doc).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(snapshot_path())
            .expect("openapi.json missing; run with UPDATE_OPENAPI=1 to generate it");
        assert_eq!(
            committed, doc,
            "openapi.json drifted; run with UPDATE_OPENAPI=1 to refresh it"
        );
    }
}
