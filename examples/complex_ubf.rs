//! Run with a configured Enduro/X environment: cargo run --example complex_ubf
use endurox_rs::{ubf_fields as f, AtmiCtx, UbfDeserialize, UbfSerialize};

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct Item {
    #[ubf(field = f::T_LONG_FLD)]
    id: i64,
    #[ubf(field = f::T_STRING_FLD)]
    name: String,
}

#[derive(Debug, PartialEq, UbfSerialize, UbfDeserialize)]
struct Message {
    #[ubf(field = f::T_UBF_FLD, nested)]
    items: Vec<Item>,
    #[ubf(field = f::T_PTR_FLD, ptr)]
    linked: Vec<Option<Item>>,
    #[ubf(field = f::T_SHORT_FLD, occ = 2)]
    flags: [i16; 2],
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = AtmiCtx::new()?;
    let mut buffer = ctx.tpalloc_ubf(256)?;
    let message = Message {
        items: vec![Item {
            id: 1,
            name: "inline".into(),
        }],
        linked: vec![
            Some(Item {
                id: 2,
                name: "pointer".into(),
            }),
            None,
        ],
        flags: [3, 4],
    };
    buffer.ubf_write(&message, true)?;
    let copy = buffer.deep_clone()?;
    drop(buffer);
    let decoded = copy.ubf_read::<Message>()?;
    assert_eq!(decoded, message);
    println!("{decoded:#?}");
    Ok(())
}
