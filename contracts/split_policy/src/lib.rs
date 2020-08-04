#![cfg_attr(not(feature = "mock"), no_std)]
#![feature(proc_macro_hygiene)]
extern crate ontio_std as ostd;
use ostd::abi::{EventBuilder, Sink, Source};
use ostd::database;
use ostd::prelude::*;
use ostd::runtime::{address, check_witness, input, ret, storage_read};
extern crate common;
use common::{TokenType, CONTRACT_COMMON};
use ostd::abi::{Decoder, Encoder};
use ostd::contract::{ong, ont, wasm};

#[cfg(test)]
mod test;

const KEY_REGISTRY_PARM: &[u8] = b"01";
const KEY_BALANCE: &[u8] = b"02";

#[derive(Encoder, Decoder, Clone)]
pub struct AddrAmt {
    to: Address,
    weight: u32,
    has_withdraw: bool,
}

#[derive(Encoder, Decoder)]
pub struct RegisterParam {
    addr_amt: Vec<AddrAmt>,
    token_type: TokenType,
    contract_addr: Option<Address>,
}

impl RegisterParam {
    fn default() -> Self {
        RegisterParam {
            addr_amt: vec![],
            token_type: TokenType::ONG,
            contract_addr: None,
        }
    }
    fn from_bytes(data: &[u8]) -> RegisterParam {
        let mut source = Source::new(data);
        let rp: RegisterParam = source.read().unwrap();
        rp
    }
}

/// register the dividend distribution strategy on the chain
///
/// `key` is also called resource_id in the other contract, used to mark the uniqueness of dividend strategy
///
/// `param_bytes` is the serialization result of RegisterParam
pub fn register(key: &[u8], param_bytes: &[u8]) -> bool {
    let param = RegisterParam::from_bytes(param_bytes);
    let data = storage_read(key.as_ref()).map(|val: Vec<u8>| val);
    assert!(data.is_none());
    match param.token_type {
        TokenType::OEP4 => {
            assert!(param.contract_addr.is_some());
        }
        _ => {}
    }
    let mut valid = false;
    for aa in param.addr_amt.iter() {
        if !valid {
            valid = check_witness(&aa.to);
            break;
        }
    }
    assert!(valid);
    database::put(generate_registry_param_key(key), param);
    EventBuilder::new()
        .string("register")
        .bytearray(key)
        .bytearray(param_bytes)
        .notify();
    true
}

/// query RegisterParam by key
pub fn get_register_param(key: &[u8]) -> RegisterParam {
    database::get::<_, RegisterParam>(generate_registry_param_key(key))
        .unwrap_or(RegisterParam::default())
}

pub fn transfer(from: &Address, key: &[u8], amt: U128) -> bool {
    let self_addr = address();
    let param = get_register_param(key);
    assert!(transfer_inner(
        from,
        &self_addr,
        amt,
        &param.token_type,
        param.contract_addr
    ));
    let balance = get_balance(key);
    let balance = balance.checked_add(amt).unwrap();
    database::put(generate_balance_key(key), balance);
    EventBuilder::new()
        .string("transfer")
        .address(from)
        .bytearray(key)
        .number(amt)
        .notify();
    true
}

pub fn get_balance(key: &[u8]) -> U128 {
    database::get::<_, U128>(generate_balance_key(key)).unwrap_or(0)
}

/// the data owner withdraw token from the contract
///
/// `key` is also called resource_id in the other contract
///
/// `addr` is the address who withdraw token, need the address signature
pub fn withdraw(key: &[u8], addr: &Address) -> bool {
    assert!(check_witness(addr));
    let mut rp = get_register_param(key);
    let ind = rp
        .addr_amt
        .iter()
        .position(|addr_amt| &addr_amt.to == addr)
        .expect("not found the addr");

    let total = rp.addr_amt.iter().fold(0, |res, x| res + x.weight);
    let addr_amt = rp.addr_amt.get_mut(ind).unwrap();
    if addr_amt.has_withdraw == false {
        let self_addr = address();
        let balance = get_balance(key);
        let temp = balance.checked_mul(addr_amt.weight as U128).unwrap();
        let amt = temp.checked_div(total as U128).unwrap();
        assert!(transfer_inner(
            &self_addr,
            &addr_amt.to,
            amt,
            &rp.token_type,
            rp.contract_addr
        ));
        addr_amt.has_withdraw = true;
        database::put(generate_registry_param_key(key), rp);
        EventBuilder::new()
            .string("withdraw")
            .bytearray(key)
            .address(addr)
            .notify();
        return true;
    } else {
        panic!("has withdraw")
    }
}

//mp invoke
pub fn transfer_withdraw(from: &Address, key: &[u8], amt: U128) -> bool {
    let mut rp = get_register_param(key);
    let total = rp.addr_amt.iter().fold(0, |res, x| res + x.weight);
    for addr_amt in rp.addr_amt.iter_mut() {
        if !addr_amt.has_withdraw {
            let temp = amt.checked_mul(addr_amt.weight as U128).unwrap();
            let temp = temp.checked_div(total as U128).unwrap();
            assert!(transfer_inner(
                from,
                &addr_amt.to,
                temp,
                &rp.token_type,
                rp.contract_addr
            ));
            addr_amt.has_withdraw = true;
        }
    }
    database::put(generate_registry_param_key(key), rp);
    EventBuilder::new()
        .string("transferWithdraw")
        .address(from)
        .bytearray(key)
        .number(amt)
        .notify();
    true
}

fn transfer_inner(
    from: &Address,
    to: &Address,
    amt: U128,
    token_type: &TokenType,
    contract: Option<Address>,
) -> bool {
    match token_type {
        TokenType::ONG => {
            assert!(ong::transfer(from, to, amt));
        }
        TokenType::ONT => {
            assert!(ont::transfer(from, to, amt));
        }
        TokenType::OEP4 => {
            //TODO
            if let Some(contract_addr) = contract {
                let res =
                    wasm::call_contract(&contract_addr, ("transfer", (from, to, amt))).unwrap();
                let mut source = Source::new(&res);
                let r: bool = source.read().unwrap();
                assert!(r);
                return true;
            }
            panic!("not reachable");
        }
    }
    true
}

fn generate_registry_param_key(key: &[u8]) -> Vec<u8> {
    [KEY_REGISTRY_PARM, key].concat()
}

fn generate_balance_key(key: &[u8]) -> Vec<u8> {
    [KEY_BALANCE, key].concat()
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
        b"register" => {
            let (key, param_bytes) = source.read().unwrap();
            sink.write(register(key, param_bytes));
        }
        b"getRegisterParam" => {
            let key = source.read().unwrap();
            sink.write(get_register_param(key));
        }
        b"transfer" => {
            let (from, key, amt): (Address, &[u8], U128) = source.read().unwrap();
            sink.write(transfer(&from, key, amt));
        }
        b"getBalance" => {
            let key = source.read().unwrap();
            sink.write(get_balance(key));
        }
        b"withdraw" => {
            let (key, addr): (&[u8], Address) = source.read().unwrap();
            sink.write(withdraw(key, &addr));
        }
        b"transferWithdraw" => {
            let (from, key, amt): (Address, &[u8], U128) = source.read().unwrap();
            sink.write(transfer_withdraw(&from, key, amt));
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
