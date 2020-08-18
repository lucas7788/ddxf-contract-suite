#![cfg_attr(not(feature = "mock"), no_std)]
#![feature(proc_macro_hygiene)]
extern crate alloc;
extern crate common;
extern crate ontio_std as ostd;
use common::CONTRACT_COMMON;
use ostd::abi::{Sink, Source};
use ostd::contract::wasm;
use ostd::database;
use ostd::prelude::*;
use ostd::runtime;
use ostd::runtime::check_witness;
use ostd::types::{Address, U128};

const MP_CONTRACT_ADDRESS: Address = ostd::macros::base58!("AdD2eNZihgt1QSy6WcxaZrxGUQi6mmx793");
const DTOKEN_CONTRACT_ADDRESS: Address =
    ostd::macros::base58!("AQJzHbcT9pti1zzV2cRZ92B1i1z8QNN2n6");

const KEY_MP_CONTRACT: &[u8] = b"01";
const KEY_DTOKEN_CONTRACT: &[u8] = b"02";

fn get_mp_contract_addr() -> Address {
    database::get::<_, Address>(KEY_MP_CONTRACT).unwrap_or(MP_CONTRACT_ADDRESS)
}

fn set_mp_contract_addr(mp: &Address) -> bool {
    assert!(check_witness(CONTRACT_COMMON.admin()));
    database::put(KEY_MP_CONTRACT, mp);
    true
}

fn get_dtoken_contract_addr() -> Address {
    database::get::<_, Address>(KEY_DTOKEN_CONTRACT).unwrap_or(DTOKEN_CONTRACT_ADDRESS)
}

fn set_dtoken_contract_addr(dtoken: &Address) -> bool {
    assert!(check_witness(CONTRACT_COMMON.admin()));
    database::put(KEY_DTOKEN_CONTRACT, dtoken);
    true
}

fn init(mp: &Address, dtoken: &Address) -> bool {
    assert!(check_witness(CONTRACT_COMMON.admin()));
    database::put(KEY_MP_CONTRACT, mp);
    database::put(KEY_DTOKEN_CONTRACT, dtoken);
    true
}

fn freeze_and_publish(
    old_resource_id: &[u8],
    new_resource_id: &[u8],
    resource_ddo_bytes: &[u8],
    item_bytes: &[u8],
    split_policy_param_bytes: &[u8],
) -> bool {
    let mp = get_mp_contract_addr();
    verify_result(wasm::call_contract(&mp, ("freeze", (old_resource_id,))));
    verify_result(wasm::call_contract(
        &mp,
        (
            "dtokenSellerPublish",
            (
                new_resource_id,
                resource_ddo_bytes,
                item_bytes,
                split_policy_param_bytes,
            ),
        ),
    ));
    true
}

pub fn buy_use_token(
    resource_id: &[u8],
    n: U128,
    buyer_account: &Address,
    payer: &Address,
    token_template_bytes: &[u8],
) -> bool {
    //call market place
    let mp = get_mp_contract_addr();
    verify_result(wasm::call_contract(
        &mp,
        ("buyDToken", (resource_id, n, buyer_account, payer)),
    ));
    //call dtoken
    let dtoken = get_dtoken_contract_addr();
    if let Some(r) = res {
        let mut source = Source::new(r.as_slice());
        let token_id: &[u8] = source.read().unwrap();
        verify_result(wasm::call_contract(
            &dtoken,
            ("useToken", (buyer_account, token_template_bytes, n)),
        ));
        true
    } else {
        false
    }
}

pub fn buy_reward_and_use_token(
    resource_id: &[u8],
    n: U128,
    buyer_account: &Address,
    payer: &Address,
    reward_uint_price: U128,
    token_template_bytes: &[u8],
) -> bool {
    //call market place
    let mp = get_mp_contract_addr();
    verify_result(wasm::call_contract(
        &mp,
        (
            "buyDTokenReward",
            (resource_id, n, buyer_account, payer, reward_uint_price),
        ),
    ));

    //call dtoken
    let dtoken = get_dtoken_contract_addr();
    verify_result(wasm::call_contract(
        &dtoken,
        ("useToken", (buyer_account, token_template_bytes, n)),
    ));
    true
}

fn buy_dtokens_and_set_agents(
    resource_ids: Vec<&[u8]>,
    ns: Vec<U128>,
    use_index: U128,
    authorized_index: U128,
    authorized_token_template_ids: &[u8],
    use_template_bytes: &[u8],
    buyer_account: &Address,
    payer: &Address,
    agent: &Address,
) -> bool {
    let mp = get_mp_contract_addr();
    let l = resource_ids.len();
    assert_eq!(l, ns.len());
    for i in 0..l {
        verify_result(wasm::call_contract(
            &mp,
            ("buyDToken", (resource_ids[i], ns[i], buyer_account, payer)),
        ));
    }
    let dtoken = get_dtoken_contract_addr();
    verify_result(wasm::call_contract(
        &dtoken,
        (
            "setTokenAgents",
            (
                resource_ids[authorized_index as usize],
                buyer_account,
                vec![agent.clone()],
                authorized_token_template_ids,
                ns[authorized_index as usize],
            ),
        ),
    ));
    verify_result(wasm::call_contract(
        &dtoken,
        (
            "useToken",
            (
                resource_ids[use_index as usize],
                buyer_account,
                use_template_bytes,
                ns[use_index as usize],
            ),
        ),
    ));
    true
}

fn verify_result(res: Option<Vec<u8>>) {
    if let Some(r) = res {
        let mut source = Source::new(r.as_slice());
        let r: bool = source.read().unwrap();
        assert!(r);
    } else {
        panic!("call contract failed")
    }
}

#[no_mangle]
fn invoke() {
    let input = runtime::input();
    let mut source = Source::new(&input);
    let action: &[u8] = source.read().unwrap();
    let mut sink = Sink::new(12);
    match action {
        b"migrate" => {
            let (code, vm_type, name, version, author, email, desc) = source.read().unwrap();
            sink.write(CONTRACT_COMMON.migrate(code, vm_type, name, version, author, email, desc));
        }
        b"setDTokenContractAddr" => {
            let dtoken = source.read().unwrap();
            sink.write(set_dtoken_contract_addr(dtoken));
        }
        b"setMpContractAddr" => {
            let mp = source.read().unwrap();
            sink.write(set_mp_contract_addr(mp));
        }
        b"getDToken" => {
            sink.write(get_dtoken_contract_addr());
        }
        b"getMP" => {
            sink.write(get_mp_contract_addr());
        }
        b"init" => {
            let (mp, dtoken) = source.read().unwrap();
            sink.write(init(mp, dtoken));
        }
        b"buyAndUseToken" => {
            let (resource_id, n, buyer_account, payer, token_template_bytes) = source.read().unwrap();
            sink.write(buy_use_token(
                resource_id,
                n,
                buyer_account,
                payer,
                token_template_bytes,
            ));
        }
        b"buyRewardAndUseToken" => {
            let (resource_id, n, buyer_account, payer, unit_price, token_template_bytes) =
                source.read().unwrap();
            sink.write(buy_reward_and_use_token(
                resource_id,
                n,
                buyer_account,
                payer,
                unit_price,
                token_template_bytes,
            ));
        }
        b"buyDtokensAndSetAgents" => {
            let (
                resource_ids,
                ns,
                use_index,
                authorized_index,
                authorized_token_template_bytes,
                use_template_bytes,
                buyer,
                payer,
                agent,
            ) = source.read().unwrap();
            sink.write(buy_dtokens_and_set_agents(
                resource_ids,
                ns,
                use_index,
                authorized_index,
                authorized_token_template_bytes,
                use_template_bytes,
                buyer,
                payer,
                agent,
            ));
        }
        _ => {
            let method = str::from_utf8(action).ok().unwrap();
            panic!("openkg contract, not support method:{}", method)
        }
    }
    runtime::ret(sink.bytes());
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
