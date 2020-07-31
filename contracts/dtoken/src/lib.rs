#![cfg_attr(not(feature = "mock"), no_std)]
#![feature(proc_macro_hygiene)]
extern crate alloc;
extern crate common;
extern crate ontio_std as ostd;
use common::*;
use ostd::abi::{EventBuilder, Sink, Source};
use ostd::database;
use ostd::prelude::*;
use ostd::runtime;
use ostd::types::{Address, U128};
mod basic;
use crate::oep8::{AppMulParam, TrFromMulParam, TrMulParam};
use crate::utils::generate_agent_key;
use basic::*;
use common::CONTRACT_COMMON;
use ostd::runtime::check_witness;

mod oep8;

#[cfg(test)]
mod test;

#[cfg(test)]
mod oep8_test;

const KEY_DDXF_CONTRACT: &[u8] = b"02";
const KEY_ADMIN: &[u8] = b"03";
const KEY_TT_ID: &[u8] = b"05";
const PRE_TT: &[u8] = b"06";
const PRE_AUTHORIZED: &[u8] = b"07";
const PRE_TOKEN_ID: &[u8] = b"08";
const PRE_TEMPLATE_ID: &[u8] = b"09";
const PRE_AGENT: &[u8] = b"10";

/// set marketplace contract address, need admin signature
///
/// only marketplace contract has the right to invoke some method
fn set_mp_contract(new_addr: &Address) -> bool {
    let admin = get_admin();
    assert!(check_witness(&admin));
    database::put(KEY_DDXF_CONTRACT, new_addr);
    true
}

/// query marketplace contract address
fn get_mp_contract() -> Address {
    database::get(KEY_DDXF_CONTRACT).unwrap_or(Address::new([0u8; 20]))
}

/// update admin address
///
/// need old admin signature
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

/// generate dtoken
///
/// when the user calls buy dtoken in marketplace contract, marketplace contract will call the generate_dtoken method of the contract to generate dtoken for the buyer
///
/// `account` is the buyer address
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `token_template_id` used to mark the only token_template
///
/// `n` represents the number of generate tokens
pub fn generate_dtoken(acc: &Address, token_template_id: &[u8], n: U128) -> bool {
    let caller = runtime::caller();
    assert!(is_valid_addr(&[&caller, acc], token_template_id));
    assert!(check_witness(acc));
    generate_dtoken_inner(acc, acc, token_template_id, n)
}

fn generate_dtoken_inner(acc: &Address, to: &Address, token_template_id: &[u8], n: U128) -> bool {
    let tt = get_token_template(token_template_id).unwrap();
    let token_id =
        oep8::generate_token(tt.token_name.as_slice(), tt.token_symbol.as_slice(), n, to);
    let key = get_key(PRE_TOKEN_ID, token_template_id);
    database::put(key.as_slice(), token_id.as_slice());
    let key = get_key(PRE_TEMPLATE_ID, token_id.as_slice());
    database::put(key.as_slice(), token_template_id);
    EventBuilder::new()
        .string("generateDToken")
        .address(acc)
        .address(to)
        .bytearray(token_template_id)
        .number(n)
        .bytearray(token_id.as_slice())
        .notify();
    true
}

pub fn generate_dtoken_for_other(
    acc: &Address,
    to: &Address,
    token_template_id: &[u8],
    n: U128,
) -> bool {
    let caller = runtime::caller();
    assert!(is_valid_addr(&[&caller, acc], token_template_id));
    assert!(check_witness(acc));
    generate_dtoken_inner(acc, to, token_template_id, n)
}

pub fn generate_dtoken_multi(acc: &Address, token_template_ids: &[Vec<u8>], n: U128) -> bool {
    for token_template_id in token_template_ids.iter() {
        assert!(generate_dtoken(acc, token_template_id, n));
    }
    true
}

fn get_token_template(token_template_id: &[u8]) -> Option<TokenTemplate> {
    let info = database::get::<_, TokenTemplateInfo>(get_key(PRE_TT, token_template_id));
    if let Some(data) = info {
        Some(TokenTemplate::from_bytes(data.token_template.as_slice()))
    } else {
        None
    }
}

pub fn create_token_template(creator: &Address, tt_bs: &[u8]) -> bool {
    assert!(check_witness(creator));
    let tt_id = get_next_tt_id();
    let tt_id_str = tt_id.to_string();
    database::put(
        get_key(PRE_TT, tt_id_str.as_bytes()),
        TokenTemplateInfo::new(creator.clone(), tt_bs.to_vec()),
    );
    update_next_tt_id(tt_id + 1);
    EventBuilder::new()
        .string("createTokenTemplate")
        .address(creator)
        .bytearray(tt_bs)
        .bytearray(tt_id_str.as_bytes())
        .notify();
    true
}

pub fn update_token_template(token_template_id: &[u8], tt_bs: &[u8]) -> bool {
    let tt_info = database::get::<_, TokenTemplateInfo>(get_key(PRE_TT, token_template_id))
        .expect("not existed token template");
    assert!(check_witness(&tt_info.creator));
    database::put(
        get_key(PRE_TT, token_template_id),
        TokenTemplateInfo::new(tt_info.creator, tt_bs.to_vec()),
    );
    EventBuilder::new()
        .string("updateTokenTemplate")
        .bytearray(token_template_id)
        .bytearray(tt_bs)
        .notify();
    true
}

pub fn remove_token_template(token_template_id: &[u8]) -> bool {
    assert!(verify_creator_sig(token_template_id));
    database::delete(get_key(PRE_TT, token_template_id));
    EventBuilder::new()
        .string("removeTokenTemplate")
        .bytearray(token_template_id)
        .notify();
    true
}

pub fn verify_creator_sig(token_template_id: &[u8]) -> bool {
    let tt_info = database::get::<_, TokenTemplateInfo>(get_key(PRE_TT, token_template_id))
        .expect("not existed token template");
    assert!(check_witness(&tt_info.creator));
    true
}

pub fn verify_creator_sig_multi(token_template_ids: &[Vec<u8>]) -> bool {
    for token_template_id in token_template_ids.iter() {
        assert!(verify_creator_sig(token_template_id));
    }
    true
}

pub fn authorize_token_template(token_template_id: &[u8], authorized_addr: &[Address]) -> bool {
    assert!(verify_creator_sig(token_template_id));
    let mut addrs = get_authorized_addr(token_template_id);
    for addr in authorized_addr.iter() {
        for add in addrs.iter() {
            if add == addr {
                continue;
            }
        }
        addrs.push(addr.clone());
    }
    let key = get_key(PRE_AUTHORIZED, token_template_id);
    database::put(key.as_slice(), addrs);
    EventBuilder::new()
        .string("authorizeTokenTemplate")
        .bytearray(token_template_id)
        .address_list(authorized_addr)
        .notify();
    true
}

pub fn remove_authorize_addr(token_template_id: &[u8], authorized_addr: &[Address]) -> bool {
    assert!(verify_creator_sig(token_template_id));
    let mut addrs = get_authorized_addr(token_template_id);
    for addr in authorized_addr.iter() {
        let index = addrs.iter().position(|x| x == addr);
        if let Some(ind) = index {
            addrs.remove(ind);
        }
    }
    let key = get_key(PRE_AUTHORIZED, token_template_id);
    database::put(key.as_slice(), addrs);
    EventBuilder::new()
        .string("removeAuthorizeAddr")
        .bytearray(token_template_id)
        .address_list(authorized_addr)
        .notify();
    true
}

pub fn authorize_token_template_multi(
    token_template_ids: &[Vec<u8>],
    authorized_addr: &[Address],
) -> bool {
    for token_template_id in token_template_ids.iter() {
        assert!(authorize_token_template(token_template_id, authorized_addr));
    }
    true
}

fn get_authorized_addr(token_template_id: &[u8]) -> Vec<Address> {
    let key = get_key(PRE_AUTHORIZED, token_template_id);
    database::get::<_, Vec<Address>>(key.as_slice()).unwrap_or(vec![])
}

fn is_valid_addr(acc: &[&Address], token_template_id: &[u8]) -> bool {
    let tt_info = database::get::<_, TokenTemplateInfo>(get_key(PRE_TT, token_template_id))
        .expect("not existed token template");
    let ind = acc.iter().position(|&x| x == &tt_info.creator);
    if ind.is_some() {
        return true;
    } else {
        let addrs = get_authorized_addr(token_template_id);
        for addr in addrs.iter() {
            let ind = acc.iter().position(|&x| x == addr);
            if ind.is_some() {
                return true;
            }
        }
    }
    false
}

fn get_token_id_by_template_id(token_template_id: &[u8]) -> Vec<u8> {
    let key = get_key(PRE_TOKEN_ID, token_template_id);
    database::get::<_, Vec<u8>>(key.as_slice()).unwrap_or(vec![])
}

fn get_template_id_by_token_id(token_id: &[u8]) -> Vec<u8> {
    let key = get_key(PRE_TEMPLATE_ID, token_id);
    database::get::<_, Vec<u8>>(key.as_slice()).unwrap_or(vec![])
}

fn get_key(pre: &[u8], post: &[u8]) -> Vec<u8> {
    [pre, post].concat()
}
fn get_next_tt_id() -> U128 {
    database::get::<_, U128>(KEY_TT_ID).unwrap_or(0)
}
fn update_next_tt_id(new_id: U128) {
    database::put(KEY_TT_ID, new_id)
}

/// use token,
///
/// the buyer of the token has the right to consume the token
///
/// `account` is the buyer address
///
/// `token_id` used to mark the only commodity in the chain
///
/// `token_template_bytes` used to mark the only token
///
/// `n` represents the number of consuming token
pub fn use_token(account: &Address, token_id: &[u8], n: U128) -> bool {
    assert!(check_witness(account));
    let ba = oep8::balance_of(account, token_id);
    assert!(ba >= n);
    oep8::destroy_token(account, token_id, n);
    EventBuilder::new()
        .string("useToken")
        .address(account)
        .bytearray(token_id)
        .number(n)
        .notify();
    true
}

fn delete_token(account: &Address, token_id: &[u8]) -> bool {
    let template_id = get_template_id_by_token_id(token_id);
    let info = database::get::<_, TokenTemplateInfo>(get_key(PRE_TT, token_template_id)).unwrap();
    assert!(
        check_witness(account) && &info.creator == account
            || check_witness(CONTRACT_COMMON.admin())
    );

    oep8::delete_token(token_id);
    let key = get_key(PRE_TT, template_id.as_slice());
    database::delete(key.as_slice());
    let key = get_key(PRE_TOKEN_ID, template_id.as_slice());
    database::delete(key.as_slice());
    let key = get_key(PRE_TEMPLATE_ID, token_id);
    database::delete(key.as_slice());
    let key = get_key(PRE_AUTHORIZED, template_id.as_slice());
    database::delete(key.as_slice());
    EventBuilder::new()
        .string("deleteToken")
        .address(account)
        .bytearray(token_id)
        .notify();
    true
}

/// use token by agent, the agent of the token has the right to invoke this method
///
/// `account` is the buyer address
///
/// `agent` is the authorized address
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `token_template_bytes` used to mark the only token
///
/// `n` represents the number of consuming token
pub fn use_token_by_agent(account: &Address, agent: &Address, token_id: &[u8], n: U128) -> bool {
    assert!(check_witness(agent));
    let ba = oep8::balance_of(account, token_id);
    assert!(ba >= n);
    let mut sink = Sink::new(64);
    generate_agent_key(&mut sink, agent, token_id);
    let agent_count = database::get::<_, U128>(sink.bytes()).unwrap_or(0);
    assert!(agent_count >= n);
    if agent_count == n {
        database::delete(sink.bytes());
    } else {
        let agent_count = agent_count.checked_sub(n).unwrap();
        database::put(sink.bytes(), agent_count);
    }
    oep8::destroy_token(account, token_id, n);
    EventBuilder::new()
        .string("useTokenByAgent")
        .address(account)
        .address(agent)
        .bytearray(token_id)
        .number(n)
        .notify();
    true
}

/// set agents, this method will set agents more than one TokeTemplate
///
/// `account` is the buyer address
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `agents` is the array of address who will be authorized agents
///
/// `n` represents the number of authorized token
///
/// `token_template_bytes` is array of TokenTemplate
pub fn set_agents(
    account: &Address,
    agents: Vec<Address>,
    n: Vec<U128>,
    token_ids: Vec<Vec<u8>>,
) -> bool {
    assert!(check_witness(account));
    for id in token_ids.iter() {
        assert!(set_token_agents_inner(
            account,
            id,
            agents.as_slice(),
            n.as_slice()
        ));
    }
    true
}

/// set token agents
///
/// `account` is the buyer address
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `token_template_bytes` used to mark the only token
///
/// `agents` is the array of address who will be authorized as agents
///
/// `n` represents the number of authorized token
pub fn set_token_agents(
    account: &Address,
    token_id: &[u8],
    agents: Vec<Address>,
    n: Vec<U128>,
) -> bool {
    assert!(check_witness(account));
    set_token_agents_inner(account, token_id, agents.as_slice(), n.as_slice())
}

pub fn set_token_agents_inner(
    account: &Address,
    token_id: &[u8],
    agents: &[Address],
    n: &[U128],
) -> bool {
    let ba = oep8::balance_of(account, token_id);
    let sum = n.iter().fold(0, |sum, &x| sum + x);
    assert!(ba >= sum);
    let mut sink = Sink::new(64);
    let l = agents.len();
    for i in 0..l {
        sink.clear();
        generate_agent_key(&mut sink, agents.get(i).unwrap(), token_id);
        database::put(sink.bytes(), n.get(i).unwrap());
        EventBuilder::new()
            .string("setTokenAgents")
            .address(account)
            .bytearray(token_id)
            .address(agents.get(i).unwrap())
            .number(*n.get(i).unwrap())
            .notify();
    }
    true
}

fn get_agent_balance(agent: &Address, token_id: &[u8]) -> U128 {
    let mut sink = Sink::new(64);
    generate_agent_key(&mut sink, agent, token_id);
    database::get::<_, U128>(sink.bytes()).unwrap_or(0)
}

/// add_agents, this method append agents for the specified token
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `account` is user address who authorize the other address is agent, need account signature
///
/// `agents` is the array of agent address
///
/// `n` is number of authorizations per agent
pub fn add_agents(
    account: &Address,
    agents: Vec<Address>,
    n: Vec<U128>,
    token_ids: Vec<Vec<u8>>,
) -> bool {
    assert!(check_witness(account));
    for id in token_ids.iter() {
        assert!(add_token_agents_inner(account, id, &agents, n.as_slice()));
    }
    true
}

/// add_agents, this method only append agents for the specified token.
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `account` is user address who authorize the other address is agent, need account signature
///
/// `token_template_bytes` used to specified which token to set agents.
///
/// `agents` is the array of agent address
///
/// `n` is number of authorizations per agent
pub fn add_token_agents(
    account: &Address,
    token_id: &[u8],
    agents: &[Address],
    n: Vec<U128>,
) -> bool {
    assert!(check_witness(account));
    add_token_agents_inner(account, token_id, agents, n.as_slice())
}

pub fn add_token_agents_inner(
    account: &Address,
    token_id: &[u8],
    agents: &[Address],
    n: &[U128],
) -> bool {
    let mut sink = Sink::new(64);
    let l = agents.len();
    for i in 0..l {
        sink.clear();
        generate_agent_key(&mut sink, agents.get(i).unwrap(), token_id);
        let ba = database::get::<_, U128>(sink.bytes()).unwrap_or(0);
        let ba = ba.checked_add(n[i]).unwrap();
        database::put(sink.bytes(), ba);
        EventBuilder::new()
            .string("addTokenAgents")
            .address(account)
            .bytearray(token_id)
            .address(agents.get(i).unwrap())
            .number(n[i])
            .notify();
    }
    true
}

/// product owner remove all the agents
///
/// `account` is user address who authorize the other address is agent, need account signature
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `agents` is the array of agent address which will be removed by account
///
/// `token_templates_bytes` the serialization result is array of TokenTemplate
pub fn remove_agents(account: &Address, agents: Vec<Address>, token_ids: Vec<Vec<u8>>) -> bool {
    assert!(check_witness(account));
    for id in token_ids.iter() {
        assert!(remove_token_agents_inner(account, id, agents.as_slice()));
    }
    true
}

/// product owner remove the agents of specified token
///
/// `account` is user address who authorize the other address is agent, need account signature
///
/// `resource_id` used to mark the only commodity in the chain
///
/// `token_template_bytes` is the serialization result of
///
/// `agents` is the array of agent address which will be removed by account
pub fn remove_token_agents(account: &Address, token_id: &[u8], agents: &[Address]) -> bool {
    assert!(check_witness(account));
    remove_token_agents_inner(account, token_id, agents)
}

pub fn remove_token_agents_inner(account: &Address, token_id: &[u8], agents: &[Address]) -> bool {
    let mut sink = Sink::new(64);
    for agent in agents.iter() {
        sink.clear();
        generate_agent_key(&mut sink, agent, token_id);
        database::delete(sink.bytes());
    }
    EventBuilder::new()
        .string("removeTokenAgents")
        .address(account)
        .bytearray(token_id)
        .notify();
    true
}

fn transfer_dtoken_multi(
    from: &Address,
    to: &Address,
    token_template_ids: &[Vec<u8>],
    n: U128,
) -> bool {
    assert!(check_witness(from));
    for token_template_id in token_template_ids.iter() {
        let token_id = get_token_id_by_template_id(token_template_id);
        assert!(oep8::transfer_inner(from, to, token_id.as_slice(), n));
    }
    true
}

fn transfer_dtoken(from: &Address, to: &Address, token_template_id: &[u8], n: U128) -> bool {
    let token_id = get_token_id_by_template_id(token_template_id);
    assert!(oep8::transfer(from, to, token_id.as_slice(), n));
    true
}

#[no_mangle]
pub fn invoke() {
    let input = runtime::input();
    let mut source = Source::new(&input);
    let action: &[u8] = source.read().unwrap();
    let mut sink = Sink::new(12);
    match action {
        //*********************admin method********************
        b"updateAdmin" => {
            let new_admin = source.read().unwrap();
            sink.write(update_admin(&new_admin));
        }
        b"getAdmin" => {
            sink.write(get_admin());
        }
        b"setMpContract" => {
            let new_addr = source.read().unwrap();
            sink.write(set_mp_contract(new_addr));
        }
        b"getMpContract" => {
            sink.write(get_mp_contract());
        }
        b"migrate" => {
            let (code, vm_type, name, version, author, email, desc) = source.read().unwrap();
            sink.write(CONTRACT_COMMON.migrate(code, vm_type, name, version, author, email, desc));
        }
        //*********************jwtToken method********************
        b"createTokenTemplate" => {
            let (creator, token_template_bs) = source.read().unwrap();
            sink.write(create_token_template(creator, token_template_bs));
        }
        b"getTokenTemplateById" => {
            let token_template_id = source.read().unwrap();
            sink.write(get_token_template(token_template_id));
        }
        b"updateTokenTemplate" => {
            let (token_template_id, token_template_bs) = source.read().unwrap();
            sink.write(update_token_template(token_template_id, token_template_bs));
        }
        b"removeTokenTemplate" => {
            let token_template_id = source.read().unwrap();
            sink.write(remove_token_template(token_template_id));
        }
        b"verifyCreatorSig" => {
            let token_template_id = source.read().unwrap();
            sink.write(verify_creator_sig(token_template_id));
        }
        b"verifyCreatorSigMulti" => {
            let token_template_ids: Vec<Vec<u8>> = source.read().unwrap();
            sink.write(verify_creator_sig_multi(token_template_ids.as_slice()));
        }
        b"authorizeTokenTemplate" => {
            let (token_template_id, authorized_addr): (&[u8], Vec<Address>) =
                source.read().unwrap();
            sink.write(authorize_token_template(
                token_template_id,
                authorized_addr.as_slice(),
            ));
        }
        b"removeAuthorizeAddr" => {
            let (token_template_id, authorized_addr): (&[u8], Vec<Address>) =
                source.read().unwrap();
            sink.write(remove_authorize_addr(
                token_template_id,
                authorized_addr.as_slice(),
            ));
        }
        b"authorizeTokenTemplateMulti" => {
            let (token_template_ids, authorized_addr): (Vec<Vec<u8>>, Vec<Address>) =
                source.read().unwrap();
            sink.write(authorize_token_template_multi(
                token_template_ids.as_slice(),
                authorized_addr.as_slice(),
            ));
        }
        b"getAuthorizedAddr" => {
            let token_template_id = source.read().unwrap();
            sink.write(get_authorized_addr(token_template_id));
        }
        b"getTokenIdByTemplateId" => {
            let token_template_id = source.read().unwrap();
            sink.write(get_token_id_by_template_id(token_template_id));
        }
        b"getTemplateIdByTokenId" => {
            let token_id = source.read().unwrap();
            sink.write(get_template_id_by_token_id(token_id));
        }
        b"generateDToken" => {
            let (account, token_template_id, n) = source.read().unwrap();
            sink.write(generate_dtoken(account, token_template_id, n));
        }
        b"generateDTokenForOther" => {
            let (account, to, token_template_id, n) = source.read().unwrap();
            sink.write(generate_dtoken_for_other(account, to, token_template_id, n));
        }
        b"generateDTokenMulti" => {
            let (account, token_template_ids, n): (&Address, Vec<Vec<u8>>, U128) =
                source.read().unwrap();
            sink.write(generate_dtoken_multi(
                account,
                token_template_ids.as_slice(),
                n,
            ));
        }
        b"deleteToken" => {
            let (account, token_id) = source.read().unwrap();
            sink.write(delete_token(account, token_id));
        }
        b"getAgentBalance" => {
            let (agent, token_id) = source.read().unwrap();
            sink.write(get_agent_balance(agent, token_id));
        }
        b"useToken" => {
            let (account, token_id, n) = source.read().unwrap();
            sink.write(use_token(account, token_id, n));
        }
        b"useTokenByAgent" => {
            let (account, agent, token_id, n) = source.read().unwrap();
            sink.write(use_token_by_agent(account, agent, token_id, n));
        }
        b"setAgents" => {
            let (account, agents, n, token_ids) = source.read().unwrap();
            sink.write(set_agents(account, agents, n, token_ids));
        }
        b"setTokenAgents" => {
            let (account, token_id, agents, n) = source.read().unwrap();
            sink.write(set_token_agents(account, token_id, agents, n));
        }
        b"addAgents" => {
            let (account, agents, n, token_ids) = source.read().unwrap();
            sink.write(add_agents(account, agents, n, token_ids));
        }
        b"addTokenAgents" => {
            let (account, token_id, agents, n): (&Address, &[u8], Vec<Address>, Vec<U128>) =
                source.read().unwrap();
            sink.write(add_token_agents(account, token_id, agents.as_slice(), n));
        }
        b"removeAgents" => {
            let (account, agents, token_ids) = source.read().unwrap();
            sink.write(remove_agents(account, agents, token_ids));
        }
        b"removeTokenAgents" => {
            let (account, token_id, agents): (&Address, &[u8], Vec<Address>) =
                source.read().unwrap();
            sink.write(remove_token_agents(account, token_id, agents.as_slice()));
        }
        //**************************mp invoke*************************
        b"transferDTokenMulti" => {
            let (from, to, token_template_ids, n): (&Address, &Address, Vec<Vec<u8>>, U128) =
                source.read().unwrap();
            sink.write(transfer_dtoken_multi(
                from,
                to,
                token_template_ids.as_slice(),
                n,
            ));
        }
        b"transferDToken" => {
            let (from, to, token_template_ids, n) = source.read().unwrap();
            sink.write(transfer_dtoken(from, to, token_template_ids, n));
        }
        //************************oep8 method*********************
        b"transfer" => {
            let (from, to, token_id, n) = source.read().unwrap();
            sink.write(oep8::transfer(from, to, token_id, n));
        }
        b"transferMulti" => {
            let param: Vec<TrMulParam> = source.read().unwrap();
            sink.write(oep8::transfer_multi(param.as_slice()));
        }
        b"name" => {
            let token_id = source.read().unwrap();
            sink.write(oep8::name(token_id));
        }
        b"symbol" => {
            let token_id = source.read().unwrap();
            sink.write(oep8::symbol(token_id));
        }
        b"totalSupply" => {
            let token_id = source.read().unwrap();
            sink.write(oep8::total_supply(token_id));
        }
        b"balanceOf" => {
            let (addr, token_id) = source.read().unwrap();
            sink.write(oep8::balance_of(addr, token_id));
        }
        b"approve" => {
            let (owner, spender, token_id, amt) = source.read().unwrap();
            sink.write(oep8::approve(owner, spender, token_id, amt));
        }
        b"approveMulti" => {
            let args: Vec<AppMulParam> = source.read().unwrap();
            sink.write(oep8::approve_multi(args.as_slice()));
        }
        b"allowance" => {
            let (owner, spender, token_id) = source.read().unwrap();
            sink.write(oep8::allowance(owner, spender, token_id));
        }
        b"transferFrom" => {
            let (spender, from, to, token_id, amt) = source.read().unwrap();
            sink.write(oep8::transfer_from(spender, from, to, token_id, amt));
        }
        b"transferFromMulti" => {
            let args: Vec<TrFromMulParam> = source.read().unwrap();
            sink.write(oep8::transfer_from_multi(args.as_slice()));
        }
        _ => {
            let method = str::from_utf8(action).ok().unwrap();
            panic!("dtoken contract, not support method:{}", method)
        }
    }
    runtime::ret(sink.bytes());
}

mod utils {
    use super::*;
    pub fn generate_agent_key(sink: &mut Sink, agent: &Address, token_id: &[u8]) {
        sink.write(PRE_AGENT);
        sink.write(agent);
        sink.write(token_id);
    }
}
