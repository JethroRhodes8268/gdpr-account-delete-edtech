use account_delete_edtech::{delete_account, DeletionInput, InfraiClient};

fn main() {
    let mut args = std::env::args().skip(1);
    let user_id = args.next().expect("usage: account-delete-demo <user-id> <key-id>");
    let key_id = args.next().expect("usage: account-delete-demo <user-id> <key-id>");
    if args.next().is_some() {
        panic!("usage: account-delete-demo <user-id> <key-id>");
    }

    let client = InfraiClient::from_env().expect("set INFRAI_API_KEY");
    let input = DeletionInput {
        user_id,
        key_id,
        sessions: Vec::new(),
    };
    let receipt = account_delete_edtech::futures_free::block_on(delete_account(&client, input)).expect("account deletion calls succeeded");
    println!("deleted {}: revoked {} sessions, key revoked={}, local record deleted={}", receipt.user_id, receipt.sessions_revoked, receipt.key_revoked, receipt.local_record_deleted);
}
