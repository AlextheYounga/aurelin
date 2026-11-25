use std::error::Error;

use __AURELIN_CRATE__::{
    database,
    entities::{post, user},
    factory, seed,
};
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder, Set};

#[tokio::test]
async fn migrated_posts_have_deterministic_factory_attributes_and_eager_loaded_owners()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("test.sqlite").display()
    );
    database::migrate(&url).await?;
    let db = database::connect(&url).await?;
    let user = user::ActiveModel {
        name: Set("Example".into()),
        email: Set("example@example.test".into()),
        password_hash: Set("not-a-login-credential".into()),
        created_at: Set(0),
        updated_at: Set(0),
        ..Default::default()
    }
    .insert(&db)
    .await?;
    let first = factory::post(user.id, 123).insert(&db).await?;
    let second = factory::post(user.id, 123).insert(&db).await?;
    assert_eq!((&first.title, &first.body), (&second.title, &second.body));
    assert!(!first.title.is_empty() && !first.body.is_empty());
    assert!(
        factory::post(user.id + 1000, 123)
            .insert(&db)
            .await
            .is_err()
    );
    let loaded = user::Entity::find()
        .find_with_related(post::Entity)
        .all(&db)
        .await?;
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].0.id, user.id);
    assert_eq!(loaded[0].1.len(), 2);
    assert!(loaded[0].1.iter().all(|post| post.user_id == user.id));

    seed::run(&url).await?;
    seed::run(&url).await?;
    let posts = post::Entity::find()
        .order_by_asc(post::Column::Id)
        .all(&db)
        .await?;
    assert_eq!(posts.len(), 4);
    assert_eq!(posts[2].title, posts[3].title);
    assert_eq!(posts[2].user_id, user.id);
    database::rollback(&url).await?;
    assert!(post::Entity::find().all(&db).await.is_err());
    database::migrate(&url).await?;
    assert!(post::Entity::find().all(&db).await?.is_empty());
    assert_eq!(user::Entity::find().all(&db).await?.len(), 1);
    database::fresh(&url).await?;
    assert!(user::Entity::find().all(&db).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn example_seeder_propagates_missing_user_error() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("test.sqlite").display()
    );
    database::migrate(&url).await?;
    assert!(
        seed::run(&url)
            .await
            .expect_err("a user is required")
            .to_string()
            .contains("existing user")
    );
    Ok(())
}
