use std::env;
use std::sync::Arc;

use unirique_node::node::Node;
use unirique_node::settings::Settings;

fn main() {
    let settings = Settings::from_arguments(env::args().skip(1));

    let node = Arc::new(Node::new(settings));
    node.run();
}