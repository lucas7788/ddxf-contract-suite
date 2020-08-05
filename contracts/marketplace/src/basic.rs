use super::ostd::abi::{Decoder, Encoder, Sink, Source};
use super::ostd::prelude::*;
use super::ostd::types::{Address, H256};
use common::{ContractCommon, Fee, TokenTemplate, CONTRACT_COMMON};
use hexutil::{read_hex, to_hex};

#[derive(Clone, Encoder, Decoder)]
pub struct ResourceDDO {
    pub manager: Address, // data owner
    pub item_meta_hash: H256,
    pub dtoken_contract_address: Option<Vec<Address>>, // can be empty
    pub mp_contract_address: Option<Address>,          // can be empty
    pub split_policy_contract_address: Option<Address>, //can be empty
}

impl ResourceDDO {
    pub fn from_bytes(data: &[u8]) -> Self {
        let mut source = Source::new(data);
        source.read().unwrap()
    }
    #[cfg(test)]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut sink = Sink::new(16);
        sink.write(self);
        sink.bytes().to_vec()
    }
}

#[derive(Encoder, Decoder, Clone)]
pub struct SellerItemInfo {
    pub item: DTokenItem,
    pub resource_ddo: ResourceDDO,
}

impl SellerItemInfo {
    pub fn new(item: DTokenItem, resource_ddo: ResourceDDO) -> Self {
        SellerItemInfo { item, resource_ddo }
    }
}

#[derive(Clone, Encoder, Decoder)]
pub struct DTokenItem {
    pub fee: Fee,
    pub expired_date: u64,
    pub stocks: u32,
    pub sold: u32,
    pub token_templates: Vec<TokenTemplate>,
}

impl DTokenItem {
    pub fn get_templates_bytes(&self) -> Vec<u8> {
        let mut sink = Sink::new(16);
        sink.write(&self.token_templates);
        sink.bytes().to_vec()
    }

    pub fn from_bytes(data: &[u8]) -> Self {
        let mut source = Source::new(data);
        source.read().unwrap()
    }

    #[cfg(test)]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut sink = Sink::new(16);
        sink.write(self);
        sink.bytes().to_vec()
    }
}

#[test]
fn test_dtoken() {

//    let data = "1ffc48df93cde46d3f80523a24e82567da725d1902010000000000000023ba3a6669350900102700000000000001012c646174615f69645f37613132383139642d316663362d343536382d626664312d6164333161383034646533610101310461616161b2fe73036679aaf4ab4be5fee9d07b3516324b2454407ca986a6636ea95689190e98ccf372b8c07a5156f2f0d13931db804fb6b7000000";
//    let bs = read_hex(data).unwrap_or_default();
//    let mut source = Source::new(bs.as_slice());
//    let rrr :SellerItemInfo= source.read().unwrap();
//    println!("{}", rrr.item.token_templates.len());

    let data = "1ffc48df93cde46d3f80523a24e82567da725d190200000000000000008cfb486669350900102700000000000001012a6469643a6f6e743a544e735351344374366f474c684c61637465586a7a454d3957736646376a47614c4c0101310461616161";
    let bs = read_hex(data).unwrap_or_default();
    let mut item = DTokenItem::from_bytes(bs.as_slice());
    item.fee.count = 0;

    let data_id = item.token_templates.get(0).unwrap().data_id.as_ref().unwrap();
    let id = str::from_utf8(data_id.as_slice()).unwrap_or_default();
    println!("id: {}", id);

    let r = to_hex(item.token_templates[0].to_bytes().as_slice());
    println!("{}", r);
    let r2 = to_hex(item.to_bytes().as_slice());
    println!("{}", r2);

    let ddo = "4f53fb01c843e398e963929c5a96c11a07a61ed4804669a4f71e49a598f34bf7b949676a803b6e9bc0bc483c160a6c1d80cbcd1c000000";
    let bs = read_hex(ddo).unwrap_or_default();
    let mut ddod = ResourceDDO::from_bytes(bs.as_slice());
    ddod.manager = CONTRACT_COMMON.admin().clone();

    let r3 = to_hex(ddod.to_bytes().as_slice());
    println!("{}", r3);
}

