#[derive(Debug, Clone)]
pub struct Node {
    value: f64,
    size: usize,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Node {
    fn new(value: f64) -> Node {
        Node {
            value,
            size: 1,
            left: None,
            right: None,
        }
    }

    fn size(&self) -> usize {
        self.size
    }
}

pub struct OrderStatisticsTree {
    root: Option<Box<Node>>,
}

impl OrderStatisticsTree {
    pub fn new() -> OrderStatisticsTree {
        OrderStatisticsTree { root: None }
    }

    fn size(&self) -> usize {
        match self.root {
            Some(ref node) => node.size(),
            None => 0,
        }
    }

    pub fn insert(&mut self, value: f64) {
        let node = self.root.take();
        self.root = self.insert_node(node, value);
    }

    fn insert_node(&mut self, node: Option<Box<Node>>, value: f64) -> Option<Box<Node>> {
        match node {
            Some(mut node) => {
                if value < node.value {
                    node.left = self.insert_node(node.left.take(), value);
                } else {
                    node.right = self.insert_node(node.right.take(), value);
                }
                node.size = node.size() + 1;
                Some(node)
            }
            None => Some(Box::new(Node::new(value))),
        }
    }

    pub fn remove(&mut self, value: f64) {
        let node = self.root.take();
        self.root = self.remove_node(node, value);
    }

    fn remove_node(&mut self, node: Option<Box<Node>>, value: f64) -> Option<Box<Node>> {
        match node {
            Some(mut node) => {
                if value < node.value {
                    node.left = self.remove_node(node.left.take(), value);
                    node.size = node.left.as_ref().map_or(0, |n| n.size())
                        + 1
                        + node.right.as_ref().map_or(0, |n| n.size());
                    Some(node)
                } else if value > node.value {
                    node.right = self.remove_node(node.right.take(), value);
                    node.size = node.left.as_ref().map_or(0, |n| n.size())
                        + 1
                        + node.right.as_ref().map_or(0, |n| n.size());
                    Some(node)
                } else {
                    if node.left.is_none() {
                        return node.right.take();
                    } else if node.right.is_none() {
                        return node.left.take();
                    } else {
                        let right = node.right.take().unwrap();
                        let (successor, right) = self.pop_min(&right);
                        let mut new_node = Box::new(Node {
                            value: successor.value,
                            size: node.size() - 1,
                            left: node.left.take(),
                            right: right,
                        });
                        new_node.size = new_node.left.as_ref().map_or(0, |n| n.size())
                            + 1
                            + new_node.right.as_ref().map_or(0, |n| n.size());
                        Some(new_node)
                    }
                }
            }
            None => None,
        }
    }

    fn pop_min<'a>(&'a mut self, node: &'a Box<Node>) -> (&Box<Node>, Option<Box<Node>>) {
        match &node.left {
            Some(left) => {
                let (min, new_left) = self.pop_min(&left);
                let mut new_node = Box::new(Node {
                    value: min.value,
                    size: node.size() - 1,
                    left: new_left,
                    right: node.right.clone(),
                });
                new_node.size = new_node.left.as_ref().map_or(0, |n| n.size())
                    + 1
                    + new_node.right.as_ref().map_or(0, |n| n.size());
                (min, Some(new_node))
            }
            None => (node, node.right.clone()),
        }
    }

    fn min_node<'a>(&'a self, node: &'a Box<Node>) -> &Box<Node> {
        match node.left {
            Some(ref left) => self.min_node(left),
            None => node,
        }
    }

    pub fn rank(&self, value: f64) -> usize {
        self.rank_node(self.root.as_ref(), value)
    }

    fn rank_node(&self, node: Option<&Box<Node>>, value: f64) -> usize {
        match node {
            Some(node) => {
                if value < node.value {
                    self.rank_node(node.left.as_ref(), value)
                } else if value > node.value {
                    node.left.as_ref().map_or(0, |node| node.size())
                        + 1
                        + self.rank_node(node.right.as_ref(), value)
                } else {
                    node.left.as_ref().map_or(0, |node| node.size())
                        + self.rank_node(node.right.as_ref(), value)
                        + 1
                }
            }
            None => 0,
        }
    }

    pub fn select(&self, rank: usize) -> Option<f64> {
        self.select_node(self.root.as_ref(), rank)
            .map(|node| node.value)
    }

    fn select_node<'a>(&'a self, node: Option<&'a Box<Node>>, rank: usize) -> Option<&Box<Node>> {
        match node {
            Some(node) => {
                let left_size = node.left.as_ref().map_or(0, |node| node.size());
                if rank < left_size {
                    self.select_node(node.left.as_ref(), rank)
                } else if rank > left_size {
                    self.select_node(node.right.as_ref(), rank - left_size - 1)
                } else {
                    Some(node)
                }
            }
            None => None,
        }
    }

    pub fn mean(&self) -> f64 {
        let sum = self.sum(self.root.as_ref());
        sum / self.size() as f64
    }

    pub fn sum(&self, node: Option<&Box<Node>>) -> f64 {
        match node {
            Some(node) => {
                let left_sum = self.sum(node.left.as_ref());
                let right_sum = self.sum(node.right.as_ref());
                node.value + left_sum + right_sum
            }
            None => 0.0,
        }
    }

    pub fn variance(&self) -> f64 {
        let mean = self.mean();
        let sum_squares = self.sum_squares(self.root.as_ref());
        sum_squares / self.size() as f64 - mean.powi(2)
    }

    pub fn sum_squares(&self, node: Option<&Box<Node>>) -> f64 {
        match node {
            Some(node) => {
                let left_sum = self.sum_squares(node.left.as_ref());
                let right_sum = self.sum_squares(node.right.as_ref());
                node.value.powi(2) + left_sum + right_sum
            }
            None => 0.0,
        }
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn median(&self) -> Option<f64> {
        let size = self.size();

        if size == 0 {
            None
        } else if size % 2 == 0 {
            let left = self.select(size / 2 - 1).unwrap();
            let right = self.select(size / 2).unwrap();
            Some((left + right) / 2.0)
        } else {
            self.select((size - 1) / 2)
        }
    }

    pub fn percentile(&self, p: f64) -> Option<f64> {
        if p < 0.0 || p > 100.0 || self.size() == 0 {
            return None;
        }

        let size = self.size();
        let max_rank = (size - 1) as f64;
        let rank = (p / 100.0 * max_rank).floor() as usize;
        let alpha = p / 100.0 * max_rank - rank as f64;

        let x_k = self.select(rank)?;
        if alpha == 0.0 {
            return Some(x_k);
        }
        let x_k1 = self.select(rank + 1)?;
        Some(x_k + alpha * (x_k1 - x_k))
    }

    pub fn max(&self) -> Option<f64> {
        match self.root {
            Some(ref node) => Some(self.max_node(node).value),
            None => None,
        }
    }

    fn max_node<'a>(&'a self, node: &'a Box<Node>) -> &Box<Node> {
        match node.right {
            Some(ref right) => self.max_node(right),
            None => node,
        }
    }

    pub fn min(&self) -> Option<f64> {
        match self.root {
            Some(ref node) => Some(self.min_node(node).value),
            None => None,
        }
    }

    pub fn insert_all<T, I>(&mut self, iter: I)
    where
        T: Into<f64>,
        I: IntoIterator<Item = T>,
    {
        for value in iter {
            let f: f64 = value.into();
            self.insert(f);
        }
    }

    pub fn empty(&mut self) {
        self.root = None;
    }
}

#[cfg(test)]
mod tests {
    use num_traits::ToPrimitive;

    use super::*;
    fn test_operations_reducer(operations: &[(char, f64)], expected: &[Option<f64>]) {
        let mut tree = OrderStatisticsTree::new();
        let mut actual = Vec::new();

        for &(op, value) in operations {
            match op {
                'i' => tree.insert(value),
                'd' => tree.remove(value),
                'm' => actual.push(tree.median()),
                'r' => actual.push(Some(tree.rank(value) as f64)),
                's' => actual.push(tree.select(value.to_usize().unwrap_or_default())),
                'v' => actual.push(Some(tree.variance())),
                't' => actual.push(Some(tree.std_dev())),
                'p' => actual.push(tree.percentile(value)),
                'x' => actual.push(tree.max()),
                'n' => actual.push(tree.min()),
                _ => {}
            }
        }

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_operations() {
        let operations = &[
            ('i', 1.0),
            ('m', 0.0),
            ('i', 2.0),
            ('m', 0.0),
            ('i', 3.0),
            ('m', 0.0),
            ('r', 2.0),
            ('s', 1.0),
            ('p', 50.0),
            ('x', 0.0),
            ('n', 0.0),
        ];
        let expected = &[
            Some(1.0),
            Some(1.5),
            Some(2.0),
            Some(2.0),
            Some(2.0),
            Some(2.0),
            Some(3.0),
            Some(1.0),
        ];
        test_operations_reducer(operations, expected);
    }

    #[test]
    fn test_stats() {
        env_logger::init();

        let values = vec![1.0, 3.0, 2.0, 4.0, 5.0];
        let mut tree = OrderStatisticsTree::new();

        for value in &values {
            tree.insert(*value);
        }
        let size = tree.size();
        let sum = tree.sum(tree.root.as_ref());
        let mean = tree.mean();
        let variance = tree.variance();
        let std_dev = tree.std_dev();
        let median = tree.median().unwrap();
        let quartile2 = tree.percentile(50.0).unwrap();
        let quartile1 = tree.percentile(25.0).unwrap();
        let quartile3 = tree.percentile(75.0).unwrap();
        let max = tree.max().unwrap();
        let min = tree.min().unwrap();

        assert_eq!(size, values.len());
        assert_eq!(sum, values.iter().sum::<f64>());
        assert_eq!(mean, values.iter().sum::<f64>() / values.len() as f64);
        assert_eq!(
            variance,
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
        );
        assert_eq!(std_dev, variance.sqrt());
        assert_eq!(median, 3.0);
        assert_eq!(quartile2, 3.0);
        assert_eq!(quartile1, 2.0);
        assert_eq!(quartile3, 4.0);
        assert_eq!(max, 5.0);
        assert_eq!(min, 1.0);
    }
}
// use num_traits::ToPrimitive;

// use std::{cmp::Ordering, fmt::Display};

// #[derive(Default, Clone, Debug)]
// pub struct Statistics<T>
// where
//     T: Copy
//         + Default
//         + Display
//         + PartialOrd
//         + ToPrimitive
//         + std::iter::Sum
//         + num_traits::FromPrimitive
//         + num_traits::NumOps,
// {
//     mean: f64,
//     median: f64,
//     sample_size: usize,
//     std_deviation: f64,
//     variance: f64,
//     pub data: Vec<T>,
// }

// // Statistics receive a vector of T (durations turned into microseconds)
// // total_size refers to the total number of data that was supposed to be received from the n ping echo requests
// // i.e., it should be equal to the number of echo requests
// impl<T> Statistics<T>
// where
//     T: Copy
//         + Default
//         + Display
//         + PartialOrd
//         + ToPrimitive
//         + std::iter::Sum
//         + num_traits::FromPrimitive
//         + num_traits::NumOps,
// {
//     pub fn new(data: Vec<T>) -> Self {
//         Statistics {
//             data,
//             ..Default::default()
//         }
//     }

//     // Calculate the basic statistics for the collection
//     pub fn calculate(&mut self) {
//         self.set_total_length();
//         if self.data.len() < 2 {
//             return;
//         }
//         self.calculate_average();
//         self.calculate_variance();
//         self.calculate_std_dev();
//         self.calculate_median();
//     }

//     fn calculate_average(&mut self) {
//         self.mean = self
//             .data
//             .iter()
//             .copied()
//             .sum::<T>()
//             .to_f64()
//             .unwrap_or_default()
//             / self.sample_size as f64;
//     }

//     fn calculate_median(&mut self) {
//         // // Clone and sort dataset
//         // let mut sorted_data = self.data.clone();
//         // sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap());

//         // let size = self.sample_size;

//         // let median = match size {
//         //     even if even % 2 == 0 => {
//         //         let fst_med = Self::select(&sorted_data, (even / 2) - 1);
//         //         let snd_med = Self::select(&sorted_data, even / 2);
//         //         match (fst_med, snd_med) {
//         //             (Some(fst), Some(snd)) => Some(
//         //                 (fst.to_f64().unwrap_or_default() + snd.to_f64().unwrap_or_default()) / 2.0,
//         //             ),
//         //             _ => None,
//         //         }
//         //     }
//         //     odd => Self::select(&sorted_data, odd / 2).map(|x| x.to_f64().unwrap_or_default()),
//         // };
//         self.median = self.calculate_quantile(50);
//     }

//     // fn calculate_quantile(&self, value: u64) -> f64 {
//     //     // Clone and sort dataset
//     //     let mut sorted_data = self.data.clone();
//     //     // sort_unstable is used here due to a clippy warning with sort and u128
//     //     sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap());
//     //     let size = self.sample_size;
//     //     let base = size as f64 * (value as f64 / 100.0);
//     //     let base_index = base.ceil() as usize;

//     //     let quantile = match base_index {
//     //         integer if integer as f64 == base => Some(
//     //             Self::select(&sorted_data, integer)
//     //                 .unwrap_or_default()
//     //                 .to_f64()
//     //                 .unwrap_or_default(),
//     //         ),
//     //         _ => {
//     //             let fst_med = Self::select(&sorted_data, (base_index) - 1);
//     //             let snd_med = Self::select(&sorted_data, base_index);
//     //             match (fst_med, snd_med) {
//     //                 (Some(fst), Some(snd)) => Some(
//     //                     (fst.to_f64().unwrap_or_default() + snd.to_f64().unwrap_or_default()) / 2.0,
//     //                 ),
//     //                 _ => None,
//     //             }
//     //         }
//     //     };

//     //     quantile.unwrap_or(0.0)
//     // }

//     fn calculate_quantile(&self, value: u64) -> f64 {
//         // Clone and sort dataset
//         let mut sorted_data = self.data.clone();
//         // sort_unstable is used here due to a clippy warning with sort and u128
//         sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap());
//         let size = self.sample_size;
//         let base = size as f64 * (value as f64 / 100.0);
//         let base_index = if 0.5 - base < 0.001 {
//             base.ceil() as usize
//         } else {
//             base.floor() as usize
//         };

//         let quantile = match base_index {
//             integer if integer as f64 == base => Some(
//                 Self::select(&sorted_data, integer)
//                     .unwrap_or_default()
//                     .to_f64()
//                     .unwrap_or_default(),
//             ),
//             _ => {
//                 let fst_med = Self::select(&sorted_data, (base_index) - 1);
//                 let snd_med = Self::select(&sorted_data, base_index);
//                 match (fst_med, snd_med) {
//                     (Some(fst), Some(snd)) => Some(
//                         (fst.to_f64().unwrap_or_default() + snd.to_f64().unwrap_or_default()) / 2.0,
//                     ),
//                     _ => None,
//                 }
//             }
//         };

//         quantile.unwrap_or(0.0)
//     }

//     fn calculate_std_dev(&mut self) {
//         self.std_deviation = self.variance.powf(0.5);
//     }

//     fn calculate_variance(&mut self) {
//         self.variance = self
//             .data
//             .iter()
//             .map(|value| ((*value).to_f64().unwrap_or_default() - self.mean).powi(2))
//             .sum::<f64>()
//             / (self.sample_size - 1) as f64;
//     }

//     fn partition(data: &[T]) -> Option<(Vec<T>, T, Vec<T>)> {
//         match data.len() {
//             0 => None,
//             _ => {
//                 let (pivot_slice, tail) = data.split_at(1);
//                 let pivot = pivot_slice[0];
//                 let (left, right) = tail.iter().fold((vec![], vec![]), |mut splits, next| {
//                     {
//                         let (ref mut left, ref mut right) = &mut splits;
//                         if next < &pivot {
//                             left.push(*next);
//                         } else {
//                             right.push(*next);
//                         }
//                     }
//                     splits
//                 });

//                 Some((left, pivot, right))
//             }
//         }
//     }

//     fn select(data: &[T], k: usize) -> Option<T> {
//         let part = Self::partition(data);

//         match part {
//             None => None,
//             Some((left, pivot, right)) => {
//                 let pivot_idx = left.len();

//                 match pivot_idx.cmp(&k) {
//                     Ordering::Equal => Some(pivot),
//                     Ordering::Greater => Self::select(&left, k),
//                     Ordering::Less => Self::select(&right, k - (pivot_idx + 1)),
//                 }
//             }
//         }
//     }

//     fn set_total_length(&mut self) {
//         self.sample_size = self.data.len();
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_statistics_integer() {
//         let data = vec![5, 2, 1, 4, 3];
//         let mut stats = Statistics::new(data);
//         stats.calculate();
//         assert_eq!(stats.mean, 3.0);
//         assert_eq!(stats.median, 3.0);
//         assert_eq!(stats.variance, 2.5);
//         assert_eq!(stats.std_deviation, 1.5811388300841898);
//     }

//     #[test]
//     fn test_statistics_float() {
//         let data = vec![2.0, 1.0, 3.0, 4.0, 5.0];
//         let mut stats = Statistics::new(data);
//         stats.calculate();
//         assert_eq!(stats.mean, 3.0);
//         assert_eq!(stats.median, 3.0);
//         assert_eq!(stats.variance, 2.5);
//         assert_eq!(stats.std_deviation, 1.5811388300841898);
//     }

//     #[test]
//     fn test_statistics_quantile() {
//         let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
//         let mut stats = Statistics::new(data);
//         stats.calculate();
//         assert_eq!(stats.calculate_quantile(10), 1.0);
//         assert_eq!(stats.calculate_quantile(20), 2.0);
//         assert_eq!(stats.calculate_quantile(50), 5.0);
//         assert_eq!(stats.calculate_quantile(80), 8.0);
//         assert_eq!(stats.calculate_quantile(90), 9.0);
//         assert_eq!(stats.calculate_quantile(100), 10.0);
//     }

//     #[test]
//     fn test_statistics_empty() {
//         let data: Vec<i32> = vec![];
//         let mut stats = Statistics::new(data);
//         stats.calculate();
//         assert_eq!(stats.mean, 0.0);
//         assert_eq!(stats.median, 0.0);
//         assert_eq!(stats.variance, 0.0);
//         assert_eq!(stats.std_deviation, 0.0);
//     }

//     #[test]
//     fn test_statistics_integer_single() {
//         let data = vec![1];
//         let mut stats = Statistics::new(data);
//         stats.calculate();
//         assert_eq!(stats.mean, 1.0);
//         assert_eq!(stats.median, 1.0);
//         assert_eq!(stats.variance, 0.0);
//         assert_eq!(stats.std_deviation, 0.0);
//     }

//     #[test]
//     fn test_statistics_float_single() {
//         let data = vec![1.0];
//         let mut stats = Statistics::new(data);
//         stats.calculate();
//         assert_eq!(stats.mean, 1.0);
//         assert_eq!(stats.median, 1.0);
//         assert_eq!(stats.variance, 0.0);
//         assert_eq!(stats.std_deviation, 0.0);
//     }

//     #[test]
//     fn test_statistics_integer_even() {
//         let data = vec![5, 2, 1, 4, 3, 6];
//         let mut stats = Statistics::new(data);
//         stats.calculate();
//         assert_eq!(stats.mean, 3.5);
//         assert_eq!(stats.median, 3.5);
//         assert_eq!(stats.variance, 2.9166666666666665);
//         assert_eq!(stats.std_deviation, 1.707825127659933);
//     }

//     #[test]
//     fn test_statistics_float_even() {
//         let data = vec![2.0, 1.0, 3.0, 4.0, 5.0, 6.0];
//         let mut stats = Statistics::new(data);
//         stats.calculate();
//         assert_eq!(stats.mean, 3.5);
//         assert_eq!(stats.median, 3.5);
//         assert_eq!(stats.variance, 2.9166666666666665);
//         assert_eq!(stats.std_deviation, 1.707825127659933);
//     }

//     #[test]
//     fn test_statistics_integer_odd() {
//         let data = vec![5, 2, 1, 4, 3];
//         let mut stats = Statistics::new(data);
//         stats.calculate();
//         assert_eq!(stats.mean, 3.0);
//         assert_eq!(stats.median, 3.0);
//         assert_eq!(stats.variance, 2.5);
//         assert_eq!(stats.std_deviation, 1.5811388300841898);
//     }
// }
