use fake::{
    Fake,
    faker::lorem::en::{Paragraph, Sentence},
    rand::{SeedableRng, rngs::StdRng},
};
use sea_orm::Set;

use crate::entities::post;

/// Make reproducible post attributes; persistence remains an explicit SeaORM operation.
pub fn post(user_id: i64, seed: u64) -> post::ActiveModel {
    let mut rng = StdRng::seed_from_u64(seed);
    post::ActiveModel {
        user_id: Set(user_id),
        title: Set(Sentence(3..6).fake_with_rng(&mut rng)),
        body: Set(Paragraph(1..3).fake_with_rng(&mut rng)),
        ..Default::default()
    }
}
