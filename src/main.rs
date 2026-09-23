use std::env;
use std::sync::Arc;

use unirique_node::node::Node;
use unirique_node::settings::Settings;

fn main() {
    let settings = Settings::from_arguments(env::args().skip(1));

    let node = Node::new(settings);
    if node.is_none() {
        return;
    }

    let node = Arc::new(node.unwrap());
    node.run();
}