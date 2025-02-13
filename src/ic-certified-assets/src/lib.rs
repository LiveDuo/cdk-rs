//! This module declares canister methods expected by the assets canister client.
pub mod rc_bytes;
pub mod state_machine;
pub mod types;
mod url_decode;

#[cfg(test)]
mod tests;

pub use crate::state_machine::StableState;
use crate::{
    rc_bytes::RcBytes,
    state_machine::{AssetDetails, EncodedAsset, State},
    types::*,
};
use candid::{candid_method, Principal, Nat};
use ic_cdk::api::{caller, data_certificate, set_certified_data, time, trap};
use ic_cdk_macros::{query, update};
use std::cell::RefCell;

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[update]
#[candid_method(update)]
fn authorize(other: Principal) {
    let caller = caller();
    STATE.with(|s| {
        if let Err(msg) = s.borrow_mut().authorize(&caller, other) {
            trap(&msg);
        }
    })
}

#[query]
#[candid_method(query)]
fn retrieve(key: Key) -> RcBytes {
    STATE.with(|s| match s.borrow().retrieve(&key) {
        Ok(bytes) => bytes,
        Err(msg) => trap(&msg),
    })
}

#[update(guard = "is_authorized")]
#[candid_method(update)]
fn store(arg: StoreArg) {
    STATE.with(move |s| {
        if let Err(msg) = s.borrow_mut().store(arg, time()) {
            trap(&msg);
        }
        set_certified_data(&s.borrow().root_hash());
    });
}

#[update(guard = "is_authorized")]
#[candid_method(update)]
fn create_batch() -> CreateBatchResponse {
    STATE.with(|s| CreateBatchResponse {
        batch_id: s.borrow_mut().create_batch(time()),
    })
}

#[update(guard = "is_authorized")]
#[candid_method(update)]
fn create_chunk(arg: CreateChunkArg) -> CreateChunkResponse {
    STATE.with(|s| match s.borrow_mut().create_chunk(arg, time()) {
        Ok(chunk_id) => CreateChunkResponse { chunk_id },
        Err(msg) => trap(&msg),
    })
}

#[update(guard = "is_authorized")]
#[candid_method(update)]
fn create_asset(arg: CreateAssetArguments) {
    STATE.with(|s| {
        if let Err(msg) = s.borrow_mut().create_asset(arg) {
            trap(&msg);
        }
        set_certified_data(&s.borrow().root_hash());
    })
}

#[update(guard = "is_authorized")]
#[candid_method(update)]
fn set_asset_content(arg: SetAssetContentArguments) {
    STATE.with(|s| {
        if let Err(msg) = s.borrow_mut().set_asset_content(arg, time()) {
            trap(&msg);
        }
        set_certified_data(&s.borrow().root_hash());
    })
}

#[update(guard = "is_authorized")]
#[candid_method(update)]
fn unset_asset_content(arg: UnsetAssetContentArguments) {
    STATE.with(|s| {
        if let Err(msg) = s.borrow_mut().unset_asset_content(arg) {
            trap(&msg);
        }
        set_certified_data(&s.borrow().root_hash());
    })
}

#[update(guard = "is_authorized")]
#[candid_method(update)]
fn delete_asset(arg: DeleteAssetArguments) {
    STATE.with(|s| {
        s.borrow_mut().delete_asset(arg);
        set_certified_data(&s.borrow().root_hash());
    });
}

#[update(guard = "is_authorized")]
#[candid_method(update)]
fn clear() {
    STATE.with(|s| {
        s.borrow_mut().clear();
        set_certified_data(&s.borrow().root_hash());
    });
}

#[update(guard = "is_authorized")]
#[candid_method(update)]
fn commit_batch(arg: CommitBatchArguments) {
    STATE.with(|s| {
        if let Err(msg) = s.borrow_mut().commit_batch(arg, time()) {
            trap(&msg);
        }
        set_certified_data(&s.borrow().root_hash());
    });
}

#[query]
#[candid_method(query)]
fn get(arg: GetArg) -> EncodedAsset {
    STATE.with(|s| match s.borrow().get(arg) {
        Ok(asset) => asset,
        Err(msg) => trap(&msg),
    })
}

#[query]
#[candid_method(query)]
fn get_chunk(arg: GetChunkArg) -> GetChunkResponse {
    STATE.with(|s| match s.borrow().get_chunk(arg) {
        Ok(content) => GetChunkResponse { content },
        Err(msg) => trap(&msg),
    })
}

#[query]
#[candid_method(query)]
fn list() -> Vec<AssetDetails> {
    STATE.with(|s| s.borrow().list_assets())
}

// #[query]
// #[candid_method(query)]
fn http_request(req: HttpRequest) -> HttpResponse {
    let certificate = data_certificate().unwrap_or_else(|| trap("no data certificate available"));

    STATE.with(|s| {
        s.borrow().http_request(
            req,
            &certificate,
            candid::Func {
                method: "http_request_streaming_callback".to_string(),
                principal: ic_cdk::id(),
            },
        )
    })
}

pub fn http_request_handle(req: HttpRequest) -> HttpResponse {
    return http_request(req);
}

// #[query]
// #[candid_method(query)]
fn http_request_streaming_callback(token: StreamingCallbackToken) -> StreamingCallbackHttpResponse {
    STATE.with(|s| {
        s.borrow()
            .http_request_streaming_callback(token)
            .unwrap_or_else(|msg| trap(&msg))
    })
}

pub fn http_request_streaming_callback_handle(token: StreamingCallbackToken) -> StreamingCallbackHttpResponse {
    return http_request_streaming_callback(token);
}

fn is_authorized() -> Result<(), String> {
    STATE.with(|s| {
        s.borrow()
            .is_authorized(&caller())
            .then(|| ())
            .ok_or_else(|| "Caller is not authorized".to_string())
    })
}

pub fn init() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.clear();
        s.authorize_unconditionally(caller());
    });
}

pub fn pre_upgrade() -> StableState {
    STATE.with(|s| s.take().into())
}

pub fn post_upgrade(stable_state: StableState) {
    STATE.with(|s| {
        *s.borrow_mut() = State::from(stable_state);
        set_certified_data(&s.borrow().root_hash());
    });
}

#[test]
fn candid_interface_compatibility() {
    use candid::utils::{service_compatible, CandidSource};
    use std::path::PathBuf;

    candid::export_service!();
    let new_interface = __export_service();

    let old_interface =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets.did");

    println!("Exported interface: {}", new_interface);

    service_compatible(
        CandidSource::Text(&new_interface),
        CandidSource::File(old_interface.as_path()),
    )
    .expect("The assets canister interface is not compatible with the assets.did file");
}

pub fn get_asset_chunk(key: &str, index: usize) -> RcBytes {
    let arg = GetChunkArg {
        index: Nat::from(index),
        key: key.to_string(),
        content_encoding: "identity".to_string(),
        sha256: None
    };
    STATE.with(|s| match s.borrow().get_chunk(arg) {
        Ok(content) => content,
        Err(msg) => trap(&msg),
    })
}

pub fn get_asset(asset_name: String) -> Vec<u8> {
	
    let arg = GetArg {
        key: asset_name.to_owned(),
        accept_encodings: vec!["identity".to_owned()]
    };
	let asset_data = get(arg);

	let mut chunk_index = 0;
	let mut chunks_all = vec![];
	let mut current_length = 0;
    while Nat::lt(&Nat::from(current_length), &asset_data.total_length) {
		let chunk = get_asset_chunk(&asset_name, chunk_index).as_ref().to_vec();
        chunks_all.extend(chunk.iter().cloned());
		current_length += chunk.len();
		chunk_index += 1;
	}
	return chunks_all;
}

pub fn store_asset(arg: StoreArg) {
    store(arg);
}
	    
pub fn delete(arg: DeleteAssetArguments) {
    delete_asset(arg);
}

pub fn list_assets() -> Vec<AssetDetails> {
    list()
}

pub fn exists(asset_name: &str) -> bool {

    let arg = GetArg {
        key: asset_name.to_owned(),
        accept_encodings: vec!["identity".to_owned()]
    };

    STATE.with(|s| match s.borrow().get(arg) {
        Ok(asset) => true,
        Err(msg) => false,
    })
}

