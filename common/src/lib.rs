#![cfg_attr(not(feature = "mock"), no_std)]
#![feature(proc_macro_hygiene)]
extern crate ontio_std as ostd;
use core::cmp::{Eq, Ord, PartialEq, PartialOrd};
use core::option::Option;
use ontio_std::abi::EventBuilder;
use ostd::abi::{Decoder, Encoder, Error, Sink, Source};
use ostd::prelude::*;
use ostd::runtime::{check_witness, contract_delete, contract_migrate};

#[cfg(test)]
mod test;

#[derive(Encoder, Decoder)]
pub struct OrderId {
    pub item_id: Vec<u8>,
    pub tx_hash: H256,
}

impl OrderId {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut sink = Sink::new(64);
        sink.write(self);
        sink.bytes().to_vec()
    }
    pub fn from_bytes(data: &[u8]) -> OrderId {
        let mut source = Source::new(data);
        let oi: OrderId = source.read().unwrap();
        oi
    }
}

#[derive(Clone, Debug, Ord, Eq, PartialEq, PartialOrd, Encoder, Decoder)]
pub struct TokenTemplate {
    pub data_id: Option<Vec<u8>>,
    pub token_hash: Vec<Vec<u8>>,
    pub endpoint: Vec<u8>, //token service endpoint
}

impl TokenTemplate {
    pub fn from_bytes(data: &[u8]) -> Self {
        let mut source = Source::new(data);
        source.read().unwrap()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut sink = Sink::new(16);
        sink.write(self);
        sink.bytes().to_vec()
    }
}

impl TokenTemplate {
    pub fn new(data_id: Option<Vec<u8>>, token_hash: Vec<Vec<u8>>, endpoint: Vec<u8>) -> Self {
        TokenTemplate {
            data_id,
            token_hash,
            endpoint,
        }
    }
}

#[derive(Encoder, Decoder, Clone)]
pub struct Fee {
    pub contract_addr: Address,
    pub contract_type: TokenType,
    pub count: u64,
}

impl Fee {
    pub fn default() -> Self {
        Fee {
            contract_addr: Address::new([0u8; 20]),
            contract_type: TokenType::ONG,
            count: 0,
        }
    }
}

#[derive(Clone)]
pub enum TokenType {
    ONT,
    ONG,
    OEP4,
}

impl Encoder for TokenType {
    fn encode(&self, sink: &mut Sink) {
        match self {
            TokenType::ONT => {
                sink.write(0u8);
            }
            TokenType::ONG => {
                sink.write(1u8);
            }
            TokenType::OEP4 => {
                sink.write(2u8);
            }
        }
    }
}

impl<'a> Decoder<'a> for TokenType {
    fn decode(source: &mut Source<'a>) -> Result<Self, Error> {
        let ty: u8 = source.read().unwrap();
        match ty {
            0u8 => Ok(TokenType::ONT),
            1u8 => Ok(TokenType::ONG),
            2u8 => Ok(TokenType::OEP4),
            _ => {
                panic!("");
            }
        }
    }
}

pub struct ContractCommon {
    admin: Address,
}

impl ContractCommon {
    const fn new(admin: Address) -> ContractCommon {
        ContractCommon { admin: admin }
    }

    pub fn admin(&self) -> &Address {
        return &self.admin;
    }

    pub fn destroy(&self) {
        assert!(check_witness(&self.admin));
        contract_delete();
    }

    pub fn migrate(
        &self,
        code: &[u8],
        vm_type: u32,
        name: &str,
        version: &str,
        author: &str,
        email: &str,
        desc: &str,
    ) -> bool {
        assert!(check_witness(&self.admin));
        let new_addr = contract_migrate(code, vm_type, name, version, author, email, desc);
        let empty_addr = Address::new([0u8; 20]);
        assert_ne!(new_addr, empty_addr);
        EventBuilder::new()
            .string("migrate")
            .address(&new_addr)
            .notify();
        true
    }
}

pub static CONTRACT_COMMON: ContractCommon =
    ContractCommon::new(ostd::macros::base58!("Aejfo7ZX5PVpenRj23yChnyH64nf8T1zbu"));

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
