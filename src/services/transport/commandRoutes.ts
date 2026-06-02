/**
 * Maps Tauri command names to velo-server HTTP route paths (under /api).
 * Kept in sync with `src-tauri/velo-server/src/email_api.rs` and `oauth_api.rs`.
 *
 * Desktop-only commands (window/tray/devtools) have no web route and are
 * handled as no-ops by the web entry — they must never reach here.
 */
export const COMMAND_ROUTES: Record<string, string> = {
  // IMAP
  imap_test_connection: "/api/imap/test_connection",
  imap_list_folders: "/api/imap/list_folders",
  imap_fetch_messages: "/api/imap/fetch_messages",
  imap_fetch_new_uids: "/api/imap/fetch_new_uids",
  imap_search_all_uids: "/api/imap/search_all_uids",
  imap_fetch_message_body: "/api/imap/fetch_message_body",
  imap_fetch_raw_message: "/api/imap/fetch_raw_message",
  imap_set_flags: "/api/imap/set_flags",
  imap_move_messages: "/api/imap/move_messages",
  imap_delete_messages: "/api/imap/delete_messages",
  imap_get_folder_status: "/api/imap/get_folder_status",
  imap_fetch_attachment: "/api/imap/fetch_attachment",
  imap_append_message: "/api/imap/append_message",
  imap_search_folder: "/api/imap/search_folder",
  imap_sync_folder: "/api/imap/sync_folder",
  imap_raw_fetch_diagnostic: "/api/imap/raw_fetch_diagnostic",
  imap_delta_check: "/api/imap/delta_check",
  // SMTP
  smtp_send_email: "/api/smtp/send",
  smtp_test_connection: "/api/smtp/test_connection",
  // OAuth proxy (token exchange/refresh)
  oauth_exchange_token: "/api/oauth/exchange_token",
  oauth_refresh_token: "/api/oauth/refresh_token",
};
