#![cfg_attr(not(feature = "mock"), no_std)]
#![feature(proc_macro_hygiene)]
extern crate alloc;
extern crate ontio_std as ostd;
use ostd::abi::{Decoder, Encoder, EventBuilder, Sink, Source};
use ostd::contract::{ong, ont, wasm};
use ostd::database;
use ostd::prelude::*;
use ostd::runtime::{address, check_witness, input, ret};
use ostd::types::{Address, U128};

mod utils;
use utils::*;

mod basic;
use basic::*;
extern crate common;
use common::{Fee, OrderId, TokenType, CONTRACT_COMMON};

#[cfg(test)]
mod test;

const MAX_PERCENTAGE: U128 = 10000;

fn set_mp(mp_account: &Address) -> bool {
    assert!(check_witness(CONTRACT_COMMON.admin()));
    database::put(utils::KEY_MP, mp_account);
    true
}

fn get_mp_account() -> Address {
    database::get::<_, Address>(utils::KEY_MP).unwrap_or(*CONTRACT_COMMON.admin())
}

/// set charging model, need mp and seller signature
///
/// `seller_acc` is seller address
///
/// `fee_split_model` is the charging model that is agreed by the seller and MP
fn set_fee_split_model(seller_acc: &Address, fsm_bytes: &[u8]) -> bool {
    let fee_split_model = FeeSplitModel::from_bytes(fsm_bytes);
    assert!(fee_split_model.weight <= MAX_PERCENTAGE as u16);
    let mp = get_mp_account();
    assert!(check_witness(seller_acc) && check_witness(&mp));
    let mp = database::get::<_, Address>(KEY_MP).unwrap();
    assert!(check_witness(&mp) && check_witness(&seller_acc));
    database::put(
        utils::generate_fee_split_model_key(seller_acc),
        fee_split_model,
    );
    EventBuilder::new()
        .string("setFeeSplitModel")
        .address(seller_acc)
        .bytearray(fsm_bytes)
        .notify();
    true
}

/// query seller's charging model by seller's address
fn get_fee_split_model(seller_acc: &Address) -> FeeSplitModel {
    database::get::<_, FeeSplitModel>(utils::generate_fee_split_model_key(seller_acc))
        .unwrap_or(FeeSplitModel { weight: 0 })
}

/// transfer fee to the contract and register the income distribution balance of this order
///
/// `order_id_bytes` is the serialization result of OrderId
///
/// `buyer_acc` is buyer address
///
/// `split_contract_address` is split contract address which register the distribution strategy
///
/// `fee` is the cost of one share
///
/// `n` is the number of shares purchased
pub fn transfer_amount(
    order_id_bytes: &[u8],
    buyer_acc: &Address,
    split_contract_address: &Address,
    fee_bytes: &[u8],
    n: U128,
) -> bool {
    assert!(check_witness(buyer_acc));
    let fee = Fee::from_bytes(fee_bytes);
    let amt = n.checked_mul(fee.count as U128).unwrap();
    let self_addr = address();
    assert!(transfer(
        buyer_acc,
        &self_addr,
        amt,
        &fee.contract_type,
        Some(fee.contract_addr.clone()),
    ));

    //store information that split_contract needs
    let info = SettleInfo {
        split_contract_addr: split_contract_address.clone(),
        fee,
        n,
    };
    database::put(utils::generate_balance_key(order_id_bytes), info);
    EventBuilder::new()
        .string("transferAmount")
        .bytearray(order_id_bytes)
        .address(buyer_acc)
        .address(split_contract_address)
        .bytearray(fee_bytes)
        .number(n)
        .notify();
    true
}

/// query settle info by order id
pub fn get_settle_info(order_id: &[u8]) -> SettleInfo {
    database::get::<_, SettleInfo>(utils::generate_balance_key(order_id))
        .unwrap_or(SettleInfo::default())
}

/// expense settlement, first transfer fee to mp, second invoke "transferWithdraw" method of split contract
///
/// `seller_acc` is the seller address, need the address signature
///
/// `order_id` is the serialization result of OrderId
pub fn settle(seller_acc: &Address, order_id: &[u8]) -> bool {
    assert!(check_witness(seller_acc));
    let self_addr = address();
    let mp = get_mp_account();
    let info = get_settle_info(order_id);

    //1. mp
    let fee_split = get_fee_split_model(seller_acc);
    let fee = info.fee;
    let total = info.n.checked_mul(fee.count as U128).unwrap();
    let mp_fee = total.checked_mul(fee_split.weight as U128).unwrap();
    let mp_amt = mp_fee.checked_div(MAX_PERCENTAGE).unwrap();
    if mp_amt != 0 {
        assert!(transfer(
            &self_addr,
            &mp,
            mp_amt,
            &fee.contract_type,
            Some(fee.contract_addr)
        ));
    }
    //2.split
    let seller_amt = total.checked_sub(mp_amt).unwrap();
    let oi = OrderId::from_bytes(order_id);
    let res = wasm::call_contract(
        &info.split_contract_addr,
        ("transferWithdraw", (&self_addr, oi.item_id, seller_amt)),
    );
    if let Some(rr) = res {
        let mut source = Source::new(rr.as_slice());
        let r: bool = source.read().unwrap();
        assert!(r);
    } else {
        panic!("call split contract failed")
    }
    database::delete(utils::generate_balance_key(order_id));
    EventBuilder::new()
        .string("settle")
        .address(seller_acc)
        .bytearray(order_id)
        .notify();
    true
}

fn transfer(
    from: &Address,
    to: &Address,
    amt: U128,
    contract_type: &TokenType,
    contract_addr: Option<Address>,
) -> bool {
    match contract_type {
        TokenType::ONG => {
            assert!(ong::transfer(from, to, amt));
        }
        TokenType::ONT => {
            assert!(ont::transfer(from, to, amt));
        }
        TokenType::OEP4 => {
            //TODO
            let contract_address = contract_addr.unwrap();
            let res =
                wasm::call_contract(&contract_address, ("transfer", (from, to, amt))).unwrap();
            let mut source = Source::new(&res);
            let b: bool = source.read().unwrap();
            assert!(b);
        }
    }
    true
}

#[no_mangle]
pub fn invoke() {
    let input = input();
    let mut source = Source::new(&input);
    let action: &[u8] = source.read().unwrap();
    let mut sink = Sink::new(12);
    match action {
        b"migrate" => {
            let (code, vm_type, name, version, author, email, desc) = source.read().unwrap();
            sink.write(CONTRACT_COMMON.migrate(code, vm_type, name, version, author, email, desc));
        }
        b"setFeeSplitModel" => {
            let (seller_acc, fee_split_model) = source.read().unwrap();
            sink.write(set_fee_split_model(seller_acc, fee_split_model));
        }
        b"getFeeSplitModel" => {
            let seller_acc = source.read().unwrap();
            sink.write(get_fee_split_model(seller_acc));
        }
        b"transferAmount" => {
            let (order_id_bytes, buyer_acc, seller_acc, fee, n) = source.read().unwrap();
            sink.write(transfer_amount(
                order_id_bytes,
                buyer_acc,
                seller_acc,
                fee,
                n,
            ));
        }
        b"getSettleInfo" => {
            let order_id_bytes = source.read().unwrap();
            sink.write(get_settle_info(order_id_bytes));
        }
        b"settle" => {
            let (seller_acc, order_id) = source.read().unwrap();
            sink.write(settle(seller_acc, order_id));
        }
        b"set_mp" => {
            let mp_addr = source.read().unwrap();
            sink.write(set_mp(mp_addr));
        }
        b"get_mp_account" => {
            sink.write(get_mp_account());
        }
        _ => {
            let method = str::from_utf8(action).ok().unwrap();
            panic!("not support method:{}", method)
        }
    }
    ret(sink.bytes());
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
