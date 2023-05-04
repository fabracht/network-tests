use super::statistics::{Node, OrderStatisticsTree};

#[derive(Debug, Clone, Copy)]
pub enum TraversalOrder {
    Inorder,
}

pub struct TreeIterator<'a> {
    current: Option<&'a Node>,
    stack: Vec<&'a Node>,
    traversal_order: TraversalOrder,
}

impl<'a> TreeIterator<'a> {
    pub fn new(tree: &'a OrderStatisticsTree, traversal_order: TraversalOrder) -> Self {
        let mut iterator = TreeIterator {
            current: None,
            stack: Vec::new(),
            traversal_order,
        };

        match traversal_order {
            TraversalOrder::Inorder => iterator.init_inorder(tree),
        }

        iterator
    }

    fn init_inorder(&mut self, tree: &'a OrderStatisticsTree) {
        self.current = tree.root();
        self.push_left_children();
    }

    fn push_left_children(&mut self) {
        while let Some(node) = self.current {
            self.stack.push(node);
            self.current = node.left();
        }
    }

    fn next_inorder(&mut self) -> Option<&'a Node> {
        if let Some(node) = self.stack.pop() {
            self.current = node.right();
            self.push_left_children();
            Some(node)
        } else {
            None
        }
    }
}

impl<'a> Iterator for TreeIterator<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        match self.traversal_order {
            TraversalOrder::Inorder => self.next_inorder(),
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_inorder_iterator() {
        let mut tree = super::OrderStatisticsTree::new();
        for i in (0..10).into_iter().rev() {
            tree.insert(i as f64);
        }
        let mut iterator = super::TreeIterator::new(&tree, super::TraversalOrder::Inorder);
        let mut rank = 1;
        while let Some(node) = iterator.next() {
            let value = node.value();
            let vrank = tree.rank(value);
            assert_eq!(rank, vrank);
            rank += 1;
        }
    }
}
