/// This crate contains the generated Protocol Buffer code for the VectorDB gRPC service.
///
/// It is generated from the `vectordb.proto` and `editing.proto` files.

// Include the generated code
pub mod vectordb {
    include!(concat!(env!("OUT_DIR"), "/vectordb.rs"));
}

pub mod editing {
    include!(concat!(env!("OUT_DIR"), "/editing.rs"));
}

// Re-export the services
pub use vectordb::vector_db_service_server;
pub use vectordb::vector_db_service_client;
pub use editing::editing_service_server;
pub use editing::editing_service_client;

// File descriptor set for reflection
pub const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/vectordb_descriptor.bin"
)); 