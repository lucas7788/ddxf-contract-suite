use super::ostd::mock::build_runtime;
use super::{
    get_balance, get_register_param, register, transfer, transfer_withdraw, withdraw, AddrAmt,
    RegisterParam,
};
use common::TokenType;
use hexutil::read_hex;
use ontio_std::abi::{Sink, Source};
use ontio_std::types::Address;

#[test]
fn test_registry3() {
    let data =
        read_hex("01fbe02b027e61a6d7602f26cfa9487fa58ef9ee72640000000002016d7e3b08bd73754a29634e02cc7a1e67f4c585c4").unwrap_or_default();
    let rp = RegisterParam::from_bytes(data.as_slice());
    let a = 0;
}

#[test]
fn test_registry() {
    let rp = RegisterParam::default();
    let mut sink = Sink::new(32);
    sink.write(&rp);
    let mut source = Source::new(sink.bytes());
    let rp2: RegisterParam = source.read().unwrap();
    assert!(&rp.addr_amt.is_empty());
    assert!(rp2.addr_amt.is_empty());
}

#[test]
fn test_registry2() {
    let addr1 = Address::repeat_byte(1);
    let addr2 = Address::repeat_byte(2);
    let aa1 = AddrAmt {
        to: addr1,
        weight: 1000,
        has_withdraw: false,
    };
    let aa2 = AddrAmt {
        to: addr2.clone(),
        weight: 9000,
        has_withdraw: false,
    };
    let contract_addr = Address::repeat_byte(3);
    let rp = RegisterParam {
        addr_amt: vec![aa1.clone(), aa2],
        token_type: TokenType::ONG,
        contract_addr: Some(contract_addr),
    };
    let mut sink = Sink::new(64);
    sink.write(rp);
    let key = b"01";

    let handle = build_runtime();
    handle.witness(&[addr1.clone()]);
    assert!(register(key, sink.bytes()));
    let param = get_register_param(key);
    assert_eq!(param.addr_amt[0].to, addr1);

    let from = Address::repeat_byte(4);
    handle.witness(&[from.clone()]);

    let call_contract = move |_addr: &Address, _data: &[u8]| -> Option<Vec<u8>> { Some(vec![1u8]) };
    handle.on_contract_call(call_contract);
    assert!(transfer(&from, key, 100));
    assert_eq!(get_balance(key), 100);

    handle.witness(&[addr1.clone()]);
    assert!(withdraw(key, &addr1));
    let rp = get_register_param(key);
    assert!(rp.addr_amt[0].has_withdraw);

    assert_eq!(rp.addr_amt[1].has_withdraw, false);

    handle.witness(&[addr2.clone()]);
    assert!(withdraw(key, &addr2));
    let rp = get_register_param(key);
    assert!(rp.addr_amt[1].has_withdraw);
}

#[test]
fn test() {
    let addr1 = Address::repeat_byte(1);
    let addr2 = Address::repeat_byte(2);
    let aa1 = AddrAmt {
        to: addr1,
        weight: 1000,
        has_withdraw: false,
    };
    let aa2 = AddrAmt {
        to: addr2.clone(),
        weight: 9000,
        has_withdraw: false,
    };

    let temp_vec = vec![aa1.clone(), aa2.clone()];
    let total = temp_vec.iter().fold(0, |res, x| res + x.weight);
    println!("total:{}", total);

    let contract_addr = Address::repeat_byte(3);
    let rp = RegisterParam {
        addr_amt: vec![aa1.clone(), aa2],
        token_type: TokenType::ONG,
        contract_addr: Some(contract_addr),
    };
    let mut sink = Sink::new(64);
    sink.write(rp);
    let key = b"01";

    let handle = build_runtime();
    handle.witness(&[addr1.clone()]);
    assert!(register(key, sink.bytes()));

    let call_contract = move |_addr: &Address, _data: &[u8]| -> Option<Vec<u8>> { Some(vec![1u8]) };
    handle.on_contract_call(call_contract);
    let from = Address::repeat_byte(4);
    handle.witness(&[from.clone()]);
    assert!(transfer_withdraw(&from, key, 10000));
    let rp = get_register_param(key);
    assert!(rp.addr_amt[0].has_withdraw);
}
