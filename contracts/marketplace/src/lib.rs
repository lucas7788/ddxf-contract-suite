//! marketplace contract
//!
//! Seller publishing process
//!
//! seller only need invoke [`dtoken_seller_publish`](fn.dtoken_seller_publish.html) method to publish products
//!
//! Buyer purchase and use process
//!
//! first of all, buyers should invoke [`buy_dtoken`](fn.buy_dtoken.html) method to buy the released products
//!
//! Second, buyer invoke the [`use_token`](fn.use_token.html) method to consume token

#![cfg_attr(not(feature = "mock"), no_std)]
#![feature(proc_macro_hygiene)]
extern crate alloc;
extern crate common;
extern crate ontio_std as ostd;
use ostd::abi::{EventBuilder, Sink, Source};
use ostd::database;
use ostd::prelude::*;
use ostd::runtime;
use ostd::types::{Address, U128};
mod basic;
use basic::*;
mod dtoken;
use common::*;
use dtoken::*;
use ostd::contract::wasm;
use ostd::runtime::{check_witness, current_txhash};

#[cfg(test)]
mod test;

const KEY_SELLER_ITEM_INFO: &[u8] = b"01";
const KEY_DTOKEN_CONTRACT: &[u8] = b"03";
const KEY_SPLIT_POLICY_CONTRACT: &[u8] = b"04";
const KEY_ADMIN: &[u8] = b"05";

//AbtTQJYKfQxq4UdygDsbLVjE8uRrJ2H3tP
//AYnhakv7kC9R5ppw65JoE2rt6xDzCjCTvD

const DEFAULT_SPLIT_CONTRACT: Address = ostd::macros::base58!("ANzKSQWm7gLvJGrnMok2hoLQAoiLmuC5wq");
const DEFAULT_DTOKEN_CONTRACT: Address =
    ostd::macros::base58!("AQJzHbcT9pti1zzV2cRZ92B1i1z8QNN2n6");

/// set dtoken contract address as the default dtoken contract address,
/// marketplace contract will invoke dtoken contract to pay the fee
/// need admin signature
/// `new_addr` is the new dtoken contract address
pub fn set_dtoken_contract(new_addr: &Address) -> bool {
    assert!(check_witness(CONTRACT_COMMON.admin()));
    database::put(KEY_DTOKEN_CONTRACT, new_addr);
    true
}

/// init contract
/// set dtoken and split contract address
pub fn init(dtoken: Address, split_policy: Address) -> bool {
    assert!(check_witness(CONTRACT_COMMON.admin()));
    database::put(KEY_DTOKEN_CONTRACT, dtoken);
    database::put(KEY_SPLIT_POLICY_CONTRACT, split_policy);
    true
}

/// query the default dtoken contract address
pub fn get_dtoken_contract() -> Address {
    database::get::<_, Address>(KEY_DTOKEN_CONTRACT).unwrap_or(DEFAULT_DTOKEN_CONTRACT)
}

/// set split contract address as the default split contract address,
///
/// When there are multiple data owners, split contract is used to set the income distribution strategy.
///
/// need admin signature
pub fn set_split_policy_contract(new_addr: &Address) -> bool {
    assert!(check_witness(CONTRACT_COMMON.admin()));
    database::put(KEY_SPLIT_POLICY_CONTRACT, new_addr);
    true
}

/// query the default split contract address
fn get_split_policy_contract() -> Address {
    database::get::<_, Address>(KEY_SPLIT_POLICY_CONTRACT).unwrap_or(DEFAULT_SPLIT_CONTRACT)
}

/// need old admin signature
///
/// update the admin address, admin has the right to set the default dtoken and split contract address
fn update_admin(new_admin: &Address) -> bool {
    let old_admin = get_admin();
    assert!(check_witness(&old_admin));
    database::put(KEY_ADMIN, new_admin);
    true
}

/// query admin address
fn get_admin() -> Address {
    database::get::<_, Address>(KEY_ADMIN).unwrap_or(*CONTRACT_COMMON.admin())
}

/// seller publish product, need seller signature
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `resource_ddo_bytes` is the result of ResourceDDO struct serialization
///
/// `item_bytes` is the result of DTokenItem struct serialization
///
/// `split_policy_param_bytes` is the result of RegisterParam struct serialization
///
/// # Example
/// ```no_run
/// use common::{Fee,TokenType};
/// let resource_id = b"resource_id";
/// let ddo = ResourceDDO {
///        token_resource_ty_endpoints: vec![],
///        item_meta_hash: H256::repeat_byte(1),
///        manager: manager.clone(),
///        dtoken_contract_address: Some(dtoken_contract_address.clone()),
///        mp_contract_address: None,
///        split_policy_contract_address: None,
///    };
/// let contract_addr = Address::repeat_byte(4);
///    let fee = Fee {
///        contract_addr,
///        contract_type: TokenType::ONG,
///        count: 0,
///    };
///    let mut templates = vec![];
///    templates.push(token_template.clone());
///    let dtoken_item = DTokenItem {
///        fee,
///        expired_date: 1,
///        stocks: 1,
///        templates,
///    };
///  let split_param = b"test";
///  assert!(supper::dtoken_seller_publish(
///        resource_id,
///        &ddo.to_bytes(),
///        &dtoken_item.to_bytes(),
///        split_param
///    ));
/// ```
pub fn dtoken_seller_publish(
    resource_id: &[u8],
    resource_ddo_bytes: &[u8],
    item_bytes: &[u8],
    split_policy_param_bytes: &[u8],
) -> bool {
    dtoken_seller_publish_inner(
        resource_id,
        resource_ddo_bytes,
        item_bytes,
        split_policy_param_bytes,
        true,
    )
}

pub fn dtoken_seller_publish_inner(
    resource_id: &[u8],
    resource_ddo_bytes: &[u8],
    item_bytes: &[u8],
    split_policy_param_bytes: &[u8],
    is_publish: bool,
) -> bool {
    let resource_ddo = ResourceDDO::from_bytes(resource_ddo_bytes);
    let item = DTokenItem::from_bytes(item_bytes);
    assert!(runtime::check_witness(&resource_ddo.manager));
    let resource =
        database::get::<_, SellerItemInfo>(utils::generate_seller_item_info_key(resource_id));
    if is_publish {
        assert!(resource.is_none());
    }
    assert_ne!(item.token_templates.len(), 0);

    let seller = SellerItemInfo::new(item.clone(), resource_ddo.clone());
    database::put(utils::generate_seller_item_info_key(resource_id), seller);

    //invoke split_policy contract
    let split_addr = get_split_policy_contract();
    let res = wasm::call_contract(
        &resource_ddo
            .split_policy_contract_address
            .unwrap_or(split_addr),
        ("register", (resource_id, split_policy_param_bytes)),
    );
    if let Some(r) = res {
        let mut source = Source::new(r.as_slice());
        let rr: bool = source.read().unwrap();
        assert!(rr);
    } else {
        panic!("call split contract register failed");
    }

    //event
    let mut method = "dtokenSellerPublish";
    if !is_publish {
        method = "update"
    }
    EventBuilder::new()
        .string(method)
        .bytearray(resource_id)
        .bytearray(resource_ddo_bytes)
        .bytearray(item_bytes)
        .notify();
    true
}

fn update(
    resource_id: &[u8],
    resource_ddo_bytes: &[u8],
    item_bytes: &[u8],
    split_policy_param_bytes: &[u8],
) -> bool {
    dtoken_seller_publish_inner(
        resource_id,
        resource_ddo_bytes,
        item_bytes,
        split_policy_param_bytes,
        false,
    )
}

fn delete(resource_id: &[u8]) -> bool {
    let item_info =
        database::get::<_, SellerItemInfo>(utils::generate_seller_item_info_key(resource_id))
            .unwrap();
    assert!(check_witness(&item_info.resource_ddo.manager));
    database::delete(utils::generate_seller_item_info_key(resource_id));
    EventBuilder::new()
        .string("delete")
        .bytearray(resource_id)
        .notify();
    true
}

pub fn get_seller_item_info(resource_id: &[u8]) -> Vec<u8> {
    let r = runtime::storage_read(utils::generate_seller_item_info_key(resource_id).as_slice())
        .map(|val: Vec<u8>| val);
    if let Some(rr) = r {
        rr
    } else {
        vec![]
    }
}

/// buy dtoken from reseller
///
/// The seller can sell what he bought before he used it
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `n` is the number of purchases
///
/// `buyer_account` is buyer address, need this address signature
///
/// `reseller_account` is reseller address, need this address signature
pub fn buy_dtoken_from_reseller(
    resource_id: &[u8],
    n: U128,
    buyer_account: &Address,
    reseller_account: &Address,
) -> bool {
    assert!(runtime::check_witness(buyer_account) && runtime::check_witness(reseller_account));
    let item_info =
        database::get::<_, SellerItemInfo>(utils::generate_seller_item_info_key(resource_id))
            .unwrap();
    let oi = OrderId {
        item_id: resource_id.to_vec(),
        tx_hash: current_txhash(),
    };
    let split_contract = get_split_policy_contract();
    assert!(transfer_fee(
        &oi,
        buyer_account,
        item_info.resource_ddo.mp_contract_address,
        &item_info
            .resource_ddo
            .split_policy_contract_address
            .unwrap_or(split_contract),
        item_info.item.fee.clone(),
        n
    ));
    if let Some(d) = item_info.resource_ddo.dtoken_contract_address {
        let l = d.len();
        for i in 0..l {
            let mut sink = Sink::new(64);
            sink.write(vec![item_info.item.token_templates.get(i)]);
            assert!(transfer_dtoken(
                d.get(i).unwrap(),
                reseller_account,
                buyer_account,
                resource_id,
                sink.bytes(),
                n
            ));
        }
    } else {
        let dtoken = get_dtoken_contract();
        assert!(transfer_dtoken(
            &dtoken,
            reseller_account,
            buyer_account,
            resource_id,
            &item_info.item.get_templates_bytes(),
            n
        ));
    }
    EventBuilder::new()
        .string("buyDtokenFromReseller")
        .bytearray(resource_id)
        .number(n)
        .address(buyer_account)
        .address(reseller_account)
        .notify();
    true
}

/// Buy more than one dtoken at a time
///
/// `resource_ids` is array of resource_id which used to mark the only commodity in the chain
///
/// `ns` is array of n which is the number of purchases. the length of resource_ids must be the same with the length of ns.
///
/// `buyer_account` is buyer address, need this address signature
pub fn buy_dtokens(
    resource_ids: Vec<&[u8]>,
    ns: Vec<U128>,
    buyer_account: &Address,
    payer: &Address,
) -> bool {
    let l = resource_ids.len();
    assert_eq!(l, ns.len());
    for i in 0..l {
        assert!(buy_dtoken(resource_ids[i], ns[i], buyer_account, payer));
    }
    true
}

fn get_token_templates(resource_id: &[u8]) -> Vec<TokenTemplate> {
    let item_info =
        database::get::<_, SellerItemInfo>(utils::generate_seller_item_info_key(resource_id))
            .unwrap();
    return item_info.item.token_templates;
}

/// buy dtoken
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `n` is the number of purchases
///
/// `buyer_account` is buyer address, need this address signature
pub fn buy_dtoken(resource_id: &[u8], n: U128, buyer_account: &Address, payer: &Address) -> bool {
    assert!(runtime::check_witness(buyer_account) && runtime::check_witness(payer));
    let mut item_info =
        database::get::<_, SellerItemInfo>(utils::generate_seller_item_info_key(resource_id))
            .unwrap();
    let now = runtime::timestamp();
    assert!(now < item_info.item.expired_date);

    assert!(item_info.item.sold < item_info.item.stocks);
    item_info.item.sold = n.checked_add(item_info.item.sold as U128).unwrap() as u32;
    assert!(item_info.item.sold <= item_info.item.stocks);
    let oi = OrderId {
        item_id: resource_id.to_vec(),
        tx_hash: current_txhash(),
    };
    assert!(transfer_fee(
        &oi,
        payer,
        item_info.resource_ddo.mp_contract_address.clone(),
        &item_info
            .resource_ddo
            .split_policy_contract_address
            .unwrap_or(get_split_policy_contract()),
        item_info.item.fee.clone(),
        n
    ));
    database::put(
        utils::generate_seller_item_info_key(resource_id),
        &item_info,
    );
    if let Some(dtoken_addr) = item_info.resource_ddo.dtoken_contract_address {
        let l = dtoken_addr.len();
        for i in 0..l {
            let mut sink = Sink::new(64);
            sink.write(vec![item_info.item.token_templates.get(i)]);
            assert!(generate_dtoken(
                &dtoken_addr[i],
                buyer_account,
                sink.bytes(),
                n
            ));
        }
    } else {
        let dtoken = get_dtoken_contract();
        assert!(generate_dtoken(
            &dtoken,
            buyer_account,
            &item_info.item.get_templates_bytes(),
            n
        ));
    }
    EventBuilder::new()
        .string("buyDtoken")
        .bytearray(resource_id)
        .number(n)
        .address(buyer_account)
        .address(payer)
        .notify();
    true
}

/// buy_dtoken_reward
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `n` is the number of purchases
///
/// `buyer_account` is buyer address, need this address signature
/// `payer` is the address who pay the fee
/// `unit_price` unit price the buyer is willing to pay
pub fn buy_dtoken_reward(
    resource_id: &[u8],
    n: U128,
    buyer_account: &Address,
    payer: &Address,
    unit_price: U128,
) -> bool {
    assert!(runtime::check_witness(buyer_account) && runtime::check_witness(payer));
    let mut item_info =
        database::get::<_, SellerItemInfo>(utils::generate_seller_item_info_key(resource_id))
            .unwrap();
    assert!(item_info.item.fee.count == 0);
    let now = runtime::timestamp();
    assert!(now < item_info.item.expired_date);

    assert!(item_info.item.sold < item_info.item.stocks);
    item_info.item.sold = n.checked_add(item_info.item.sold as U128).unwrap() as u32;
    assert!(item_info.item.sold <= item_info.item.stocks);
    let oi = OrderId {
        item_id: resource_id.to_vec(),
        tx_hash: current_txhash(),
    };
    let mut fee = item_info.item.fee.clone();
    fee.count = unit_price as u64;
    assert!(transfer_fee(
        &oi,
        payer,
        item_info.resource_ddo.mp_contract_address.clone(),
        &item_info
            .resource_ddo
            .split_policy_contract_address
            .unwrap_or(get_split_policy_contract()),
        fee,
        n
    ));
    database::put(
        utils::generate_seller_item_info_key(resource_id),
        &item_info,
    );
    if let Some(dtoken_addr) = item_info.resource_ddo.dtoken_contract_address {
        let l = dtoken_addr.len();
        for i in 0..l {
            let mut sink = Sink::new(64);
            sink.write(vec![item_info.item.token_templates.get(i)]);
            assert!(generate_dtoken(
                &dtoken_addr[i],
                buyer_account,
                sink.bytes(),
                n
            ));
        }
    } else {
        let dtoken = get_dtoken_contract();
        assert!(generate_dtoken(
            &dtoken,
            buyer_account,
            &item_info.item.get_templates_bytes(),
            n
        ));
    }
    EventBuilder::new()
        .string("buyDtokenReward")
        .bytearray(resource_id)
        .number(n)
        .address(buyer_account)
        .address(payer)
        .notify();
    true
}

// inner method
fn transfer_fee(
    oi: &OrderId,
    buyer_account: &Address,
    mp_contract_address: Option<Address>,
    split_contract_address: &Address,
    fee: Fee,
    n: U128,
) -> bool {
    let res = match mp_contract_address {
        Some(mp_addr) => wasm::call_contract(
            &mp_addr,
            (
                "transferAmount",
                (oi.to_bytes(), buyer_account, split_contract_address, fee, n),
            ),
        ),
        _ => {
            let amt = n.checked_mul(fee.count as U128).unwrap();
            wasm::call_contract(
                split_contract_address,
                (
                    "transferWithdraw",
                    (buyer_account, oi.item_id.as_slice(), amt),
                ),
            )
        }
    };
    verify_result(res);
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
pub fn invoke() {
    let input = runtime::input();
    let mut source = Source::new(&input);
    let action: &[u8] = source.read().unwrap();
    let mut sink = Sink::new(12);
    match action {
        b"updateAdmin" => {
            let new_admin = source.read().unwrap();
            sink.write(update_admin(&new_admin));
        }
        b"getAdmin" => {
            sink.write(get_admin());
        }
        b"init" => {
            let (dtoken, split_policy) = source.read().unwrap();
            sink.write(init(dtoken, split_policy));
        }
        b"setSplitPolicyContract" => {
            let new_addr = source.read().unwrap();
            sink.write(set_split_policy_contract(new_addr));
        }
        b"getSplitPolicyContract" => sink.write(get_split_policy_contract()),
        b"setDTokenContract" => {
            let new_addr = source.read().unwrap();
            sink.write(set_dtoken_contract(&new_addr));
        }
        b"getDTokenContract" => {
            sink.write(get_dtoken_contract());
        }
        b"migrate" => {
            let (code, vm_type, name, version, author, email, desc) = source.read().unwrap();
            sink.write(CONTRACT_COMMON.migrate(code, vm_type, name, version, author, email, desc));
        }
        b"update" => {
            let (resource_id, resource_ddo, item, split_policy_param_bytes) =
                source.read().unwrap();
            sink.write(update(
                resource_id,
                resource_ddo,
                item,
                split_policy_param_bytes,
            ));
        }
        b"delete" => {
            let resource_id = source.read().unwrap();
            sink.write(delete(resource_id));
        }
        b"dtokenSellerPublish" => {
            let (resource_id, resource_ddo, item, split_policy_param_bytes) =
                source.read().unwrap();
            sink.write(dtoken_seller_publish(
                resource_id,
                resource_ddo,
                item,
                split_policy_param_bytes,
            ));
        }
        b"getSellerItemInfo" => {
            let resource_id = source.read().unwrap();
            sink.write(get_seller_item_info(resource_id))
        }
        b"buyDtokenFromReseller" => {
            let (resource_id, n, buyer_account, reseller_account) = source.read().unwrap();
            sink.write(buy_dtoken_from_reseller(
                resource_id,
                n,
                buyer_account,
                reseller_account,
            ));
        }
        b"buyDtokens" => {
            let (resource_ids, ns, buyer, payer) = source.read().unwrap();
            sink.write(buy_dtokens(resource_ids, ns, buyer, payer));
        }
        b"buyDtoken" => {
            let (resource_id, n, buyer_account, payer) = source.read().unwrap();
            sink.write(buy_dtoken(resource_id, n, buyer_account, payer));
        }
        b"buyDtokenReward" => {
            let (resource_id, n, buyer_account, payer, unit_price) = source.read().unwrap();
            sink.write(buy_dtoken_reward(
                resource_id,
                n,
                buyer_account,
                payer,
                unit_price,
            ));
        }
        b"getTokenTemplates" => {
            let resource_id = source.read().unwrap();
            sink.write(get_token_templates(resource_id));
        }
        _ => {
            let method = str::from_utf8(action).ok().unwrap();
            panic!("marketplace contract, not support method:{}", method)
        }
    }
    runtime::ret(sink.bytes());
}

mod events {
    use super::*;
    use ostd::macros::event;
    #[event(dtokenSellerPublishEvent)]
    pub fn dtoken_seller_publish_event(resource_id: &[u8], resource_ddo: &[u8], item: &[u8]) {}
    #[event(buyDtokenFromReseller)]
    pub fn buy_dtoken_from_reseller(
        resource_id: &str,
        n: U128,
        buyer_account: &Address,
        reseller_account: &Address,
    ) {
    }
    #[event(buyDtoken)]
    pub fn buy_dtoken(resource_id: &[u8], n: U128, buyer_account: &Address) {}
    #[event(useToken)]
    pub fn use_token(resource_id: &[u8], account: &Address, token_template_bytes: &[u8], n: U128) {}
    #[event(useTokenByAgent)]
    pub fn use_token_by_agent(
        resource_id: &[u8],
        account: &Address,
        agent: &Address,
        token_template_bytes: &[u8],
        n: U128,
    ) {
    }
    #[event(setDtokenAgents)]
    pub fn set_agents(resource_id: &[u8], account: &Address, n: U128) {}
}

mod utils {
    use super::*;
    use alloc::vec::Vec;
    pub fn generate_seller_item_info_key(resource_id: &[u8]) -> Vec<u8> {
        [KEY_SELLER_ITEM_INFO, resource_id].concat()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
