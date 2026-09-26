use std::env;
use std::sync::Arc;

use node::{Node, Settings};

fn main() {
    let settings = Settings::from_arguments(env::args().skip(1));

    let node = Node::new(settings);
    if node.is_none() {
        return;
    }

    let node = Arc::new(node.unwrap());
    node.run();
}
