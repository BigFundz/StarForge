use async_graphql::{Schema, EmptyMutation};
use crate::graphql::resolvers::{Query, Mutation};
use crate::graphql::subscription::Subscription;

pub type StarforgeSchema = Schema<Query, Mutation, Subscription>;

pub fn build_schema() -> StarforgeSchema {
    Schema::build(Query, Mutation, Subscription).finish()
}
