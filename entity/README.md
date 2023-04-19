# Entity

`struct` representations of the database models and DB queries go into this library.

## Where does querying logic go?

For the most part, they go into `*_queries.rs`. The times where you place querying logic in an `impl Model {}` function is when you are doing a simple retrieval of data related to an instance of a model, or you are generating the next `ActiveModel` in a historical chain (ex: `trade`, `transaction`).
