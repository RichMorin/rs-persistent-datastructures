// Copyright (c) 2013 Michael Woerister
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
use extra::arc::Arc;
use persistent::PersistentMap;

#[deriving(Clone, Eq)]
enum Color {
    NegativeBlack,
    Red,
    Black,
    DoubleBlack
}

impl Color {
    fn inc(&self) -> Color {
        match *self {
            NegativeBlack => Red,
            Red => Black,
            Black => DoubleBlack,
            DoubleBlack => fail!("Can't inc DoubleBlack")
        }
    }

    fn dec(&self) -> Color {
        match *self {
            NegativeBlack => fail!("Can't dec NegativeBlack"),
            Red => NegativeBlack,
            Black => Red,
            DoubleBlack => Black
        }
    }
}

struct NodeData<K, V> {
    left: NodeRef<K, V>,
    key: K,
    val: V,
    right: NodeRef<K, V>,
}

struct NodeRef<K, V> {
    col: Color,
    data: Option<Arc<NodeData<K, V>>>
}

impl<K, V> Clone for NodeRef<K, V> {
    fn clone(&self) -> NodeRef<K, V> {
        NodeRef {
            col: self.col,
            data: self.data.clone()
        }
    }
}

fn new_node<K: Ord+Clone+Send+Freeze, V: Clone+Send+Freeze>(color: Color,
                  left: NodeRef<K, V>,
                  key: K,
                  val: V,
                  right: NodeRef<K, V>)
               -> NodeRef<K, V> {
    let node = NodeRef {
        col: color,
        data: Some(
            Arc::new(
                NodeData {
                    left: left,
                    key: key,
                    val: val,
                    right: right
                }
            )
        )
    };
    assert!(!node.is_leaf());
    return node;
}

fn new_leaf<K: Ord+Clone+Send+Freeze, V: Clone+Send+Freeze>(color: Color) -> NodeRef<K, V> {
    assert!(color == Black || color == DoubleBlack);
    let leaf = NodeRef { col: color, data: None };
    assert!(leaf.is_leaf());
    return leaf;
}

impl<K: Ord+Clone+Send+Freeze, V: Clone+Send+Freeze> NodeRef<K, V> {

    fn is_leaf(&self) -> bool {
        self.data.is_none()
    }

    fn children(&self) -> (NodeRef<K, V>, NodeRef<K, V>) {
        match self.data {
            Some(ref data_ref) => {
                let data_ref = data_ref.get();
                (data_ref.left.clone(), data_ref.right.clone())
            }
            None => unreachable!()
        }
    }

    fn get_data<'a>(&'a self) -> &'a NodeData<K, V> {
        match self.data {
            Some(ref data_ref) => data_ref.get(),
            None => unreachable!()
        }
    }

    fn children_and_key(&self) -> (NodeRef<K, V>, K, NodeRef<K, V>) {
        match self.data {
            Some(ref data_ref) => {
                let data_ref = data_ref.get();
                (data_ref.left.clone(), data_ref.key.clone(), data_ref.right.clone())
            }
            None => unreachable!()
        }
    }

    // (define/match (redden node)
    //   [(T cmp _ l k v r)   (T cmp 'R l k v r)]
    //   [(L _)               (error "Can't redden leaf.")])
    fn redden(self) -> NodeRef<K, V> {
        assert!(!self.is_leaf());
        NodeRef {
            col: Red,
            data: self.data
        }
    }

    // (define/match (blacken node)
    // [(T cmp _ l k v r)  (T cmp 'B l k v r)]
    // [(BBL cmp)          (L cmp)]
    // [(L _)              node])
    fn blacken(self) -> NodeRef<K, V> {
        NodeRef {
            col: Black,
            data: self.data
        }
    }

    fn inc(mut self) -> NodeRef<K, V> {
        self.col = self.col.inc();
        self
    }

    fn dec(mut self) -> NodeRef<K, V> {
        self.col = self.col.dec();
        self
    }

    fn find<'a>(&'a self, search_key: &K) -> Option<&'a V> {
        match self.data {
            Some(ref data_ref) => {
                let data_ref = data_ref.get();

                if *search_key < data_ref.key {
                    data_ref.left.find(search_key)
                } else if *search_key > data_ref.key {
                    data_ref.right.find(search_key)
                } else {
                    Some(&data_ref.val)
                }
            }
            None => None
        }
    }

    // Calculates the max black nodes on path:
    fn count_black_height(&self, combine: |u64, u64| -> u64) -> u64 {
        assert!(self.col == Red || self.col == Black);

        match self.data {
            // [(T! c l r)
            Some(ref data_ref) => {
                let data_ref = data_ref.get();
                // (+ (if (eq? c 'B) 1 0) (max (max-black-height l)
                //                  (max-black-height r)))
                let this = if self.col == Black { 1 } else { 0 };
                let sub = combine(data_ref.left.count_black_height(|a, b| combine(a, b)),
                                  data_ref.right.count_black_height(|a, b| combine(a, b)));
                this + sub
            }
            // [(L!)     1]
            None => { 1 }
        }
    }

    // Does this tree contain a red child of red?
    fn no_red_red(&self) -> bool {
        assert!(self.col == Red || self.col == Black);
        if !self.is_leaf() {
            let (l, r) = self.children();
            assert!(l.col == Red || l.col == Black);
            assert!(r.col == Red || r.col == Black);

            if self.col == Black {
                return l.no_red_red() && r.no_red_red();
            }

            if self.col == Red && l.col == Black && r.col == Black {
                return l.no_red_red() && r.no_red_red();
            }

            return false;
        } else {
            return true;
        }
    }

    // Is this tree black-balanced?
    fn black_balanced(&self) -> bool {
        self.count_black_height(::std::num::max) == self.count_black_height(::std::num::min)
    }

    // Returns the maxium (key . value) pair:
    fn find_max_kvp(&self) -> (K, V) {
        assert!(!self.is_leaf());
        let data = self.get_data();
        if data.right.is_leaf() {
            (data.key.clone(), data.val.clone())
        } else {
            data.right.find_max_kvp()
        }
    }

    fn modify_at(&self, key: K, val: V, insertion_count: &mut uint) -> NodeRef<K, V> {
        //(blacken (internal-modify-at node key f)))
        self.modify_at_rec(key, val, insertion_count).blacken()
    }

    fn modify_at_rec(&self, key: K, val: V, insertion_count: &mut uint) -> NodeRef<K, V> {
        if self.is_leaf() {
            assert!(self.col == Black);
            *insertion_count = 1;
            // (T cmp 'R node key (f key #f) node)]))
            new_node(Red, (*self).clone(), key, val, (*self).clone())
        } else {
            // matches: (T cmp c l k v r)
            let k = self.get_data().key.clone();
            let v = self.get_data().val.clone();
            let c = self.col;
            let (l, r) = self.children();

            if key < k {
                // (balance cmp c (internal-modify-at l key f) k v r)
                new_node(c, l.modify_at_rec(key, val, insertion_count), k, v, r).balance()
            } else if key > k {
                // (balance cmp c l k v (internal-modify-at r key f))])
                new_node(c, l, k, v, r.modify_at_rec(key, val, insertion_count)).balance()
            } else {
                *insertion_count = 0;
                // (T cmp c l k (f k v) r)
                new_node(c, l, k, val, r)
            }
        }
    }

    // WAT!
    fn balance(self) -> NodeRef<K, V> {
        assert!(!self.is_leaf());

        if self.col == Black || self.col == DoubleBlack {
            let result_col = self.col.dec();
            let node_data = self.get_data();
            let (left_child, right_child) = self.children();

            if left_child.col == Red {
                assert!(!left_child.is_leaf());
                // (T! (or 'B 'BB) (R (R a xk xv b) yk yv c) zk zv d)
                // (T! (or 'B 'BB) (R a xk xv (R b yk yv c)) zk zv d)
                let (left_grand_child, right_grand_child) = left_child.children();

                if left_grand_child.col == Red {
                    assert!(!left_grand_child.is_leaf());
                    // (T! (or 'B 'BB) (R (R a xk xv b) yk yv c) zk zv d)
                    let (a, b) = left_grand_child.children();
                    let xk = left_grand_child.get_data().key.clone();
                    let xv = left_grand_child.get_data().val.clone();
                    let yk = left_child.get_data().key.clone();
                    let yv = left_child.get_data().val.clone();
                    let c = right_grand_child;
                    let zk = node_data.key.clone();
                    let zv = node_data.val.clone();
                    let d = right_child;
                    return create_case1(result_col, a, b, c, d, xk, xv, yk, yv, zk, zv);
                }

                if right_grand_child.col == Red {
                    assert!(!right_grand_child.is_leaf());
                    // (T! (or 'B 'BB) (R a xk xv (R b yk yv c)) zk zv d)
                    let a = left_grand_child;
                    let xk = left_child.get_data().key.clone();
                    let xv = left_child.get_data().val.clone();
                    let (b, c) = right_grand_child.children();
                    let yk = right_grand_child.get_data().key.clone();
                    let yv = right_grand_child.get_data().val.clone();
                    let zk = node_data.key.clone();
                    let zv = node_data.val.clone();
                    let d = right_child;
                    return create_case1(result_col, a, b, c, d, xk, xv, yk, yv, zk, zv);
                }
            }

            if right_child.col == Red {
                assert!(!right_child.is_leaf());
                // (T! (or 'B 'BB) a xk xv (R (R b yk yv c) zk zv d))
                // (T! (or 'B 'BB) a xk xv (R b yk yv (R c zk zv d))))
                let a = left_child;
                let xk = node_data.key.clone();
                let xv = node_data.val.clone();
                let (left_grand_child, right_grand_child) = right_child.children();

                if left_grand_child.col == Red {
                    assert!(!left_grand_child.is_leaf());
                    // (T! (or 'B 'BB) a xk xv (R (R b yk yv c) zk zv d))
                    let (b, c) = left_grand_child.children();
                    let yk = left_grand_child.get_data().key.clone();
                    let yv = left_grand_child.get_data().val.clone();
                    let zk = right_child.get_data().key.clone();
                    let zv = right_child.get_data().val.clone();
                    let d = right_grand_child;
                    return create_case1(result_col, a, b, c, d, xk, xv, yk, yv, zk, zv);
                }

                if right_grand_child.col == Red {
                    assert!(!right_grand_child.is_leaf());
                    // (T! (or 'B 'BB) a xk xv (R b yk yv (R c zk zv d))))
                    let b = left_grand_child;
                    let yk = right_child.get_data().key.clone();
                    let yv = right_child.get_data().val.clone();
                    let (c, d) = right_grand_child.children();
                    let zk = right_grand_child.get_data().key.clone();
                    let zv = right_grand_child.get_data().val.clone();
                    return create_case1(result_col, a, b, c, d, xk, xv, yk, yv, zk, zv);
                }
            }
        }

        if self.col == DoubleBlack {
            // (BB a xk xv (-B (B b yk yv c) zk zv (and d (B))))
            let (a, right_child) = self.children();
            let xk = self.get_data().key.clone();
            let xv = self.get_data().val.clone();

            if right_child.col == NegativeBlack {
                assert!(!right_child.is_leaf());
                let (left_grand_child, right_grand_child) = right_child.children();

                if !left_grand_child.is_leaf() &&
                   left_grand_child.col == Black &&
                   right_grand_child.col == Black {
                    let (b, c) = left_grand_child.children();
                    let yk = left_grand_child.get_data().key.clone();
                    let yv = left_grand_child.get_data().val.clone();
                    let zk = right_child.get_data().key.clone();
                    let zv = right_child.get_data().val.clone();
                    let d = right_grand_child;

                    // (T cmp 'B (T cmp 'B a xk xv b) yk yv (balance cmp 'B c zk zv (redden d)))
                    return new_node(
                        Black,
                        new_node(Black, a, xk, xv, b),
                        yk,
                        yv,
                        new_node(Black, c, zk, zv, d.redden()).balance()
                    );
                }
            }
        }

        if self.col == DoubleBlack {
            // (BB (-B (and a (B)) xk xv (B b yk yv c)) zk zv d)
            let (left_child, d) = self.children();

            if left_child.col == NegativeBlack {
                assert!(!left_child.is_leaf());
                let (left_grand_child, right_grand_child) = left_child.children();

                if left_grand_child.col == Black &&
                   !right_grand_child.is_leaf() &&
                   right_grand_child.col == Black {
                    let a = left_grand_child;
                    let xk = left_child.get_data().key.clone();
                    let xv = left_child.get_data().val.clone();
                    let (b, c) = right_grand_child.children();
                    let yk = right_grand_child.get_data().key.clone();
                    let yv = right_grand_child.get_data().val.clone();
                    let zk = self.get_data().key.clone();
                    let zv = self.get_data().val.clone();

                    // (T cmp 'B (balance cmp 'B (redden a) xk xv b) yk yv (T cmp 'B c zk zv d))]
                    return new_node(
                        Black,
                        new_node(Black, a.redden(), xk, xv, b).balance(),
                        yk,
                        yv,
                        new_node(Black, c, zk, zv, d)
                    );
                }
            }
        }

        return self;

        fn create_case1<K: Ord+Clone+Send+Freeze, V: Clone+Send+Freeze>(
            color: Color,
            a: NodeRef<K, V>,
            b: NodeRef<K, V>,
            c: NodeRef<K, V>,
            d: NodeRef<K, V>,
            xk: K,
            xv: V,
            yk: K,
            yv: V,
            zk: K,
            zv: V)
         -> NodeRef<K, V> {
            // (T cmp (black-1 (T-color node)) (T cmp 'B a xk xv b) yk yv (T cmp 'B c zk zv d))
            return new_node(color, new_node(Black, a, xk, xv, b), yk, yv, new_node(Black, c, zk, zv, d));
        }
    }

    // ; Deletes a key from this map:
    fn delete(&self, search_key: &K, removal_count: &mut uint) -> NodeRef<K, V> {
        // Finds the node to be removed:
        fn del<K: Ord+Clone+Send+Freeze, V: Clone+Send+Freeze>(
            node: NodeRef<K, V>,
            search_key: &K,
            removal_count: &mut uint)
         -> NodeRef<K, V> {
            if !node.is_leaf() {
                let c = node.col;
                let k = node.get_data().key.clone();
                let v = node.get_data().val.clone();
                let (l, r) = node.children();

                if *search_key < k {
                    bubble(c, del(l, search_key, removal_count), k, v, r)
                } else if *search_key > k {
                    bubble(c, l, k, v, del(r, search_key, removal_count))
                } else {
                    *removal_count = 1;
                    remove(node)
                }
            } else {
                *removal_count = 0;
                node
            }
        }

        // Removes this node; it might
        // leave behind a double-black node:
        fn remove<K: Ord+Clone+Send+Freeze, V: Clone+Send+Freeze>(node: NodeRef<K, V>) -> NodeRef<K, V> {
            assert!(!node.is_leaf());
            // Leaves are easiest to kill:
            let (left, right) = node.children();
            if left.is_leaf() && right.is_leaf() {
                return if node.col == Red {
                    new_leaf(Black)
                } else {
                    assert!(node.col == Black);
                    new_leaf(DoubleBlack)
                };
            }

            // Killing a node with one child;
            // parent or child is red:
            //    [(or (R child (L!))
            //         (R (L!) child))
            // ; =>
            // child]
            if node.col == Red {
                if right.is_leaf() {
                    return left;
                }

                if left.is_leaf() {
                    return right;
                }
            }

            if node.col == Black {
                // [(or (B (R l k v r) (L!))
                if left.col == Red && right.is_leaf() {
                    let data = left.get_data();
                    // (T cmp 'B l k v r)]
                    return new_node(Black, data.left.clone(), data.key.clone(), data.val.clone(), data.right.clone());
                }

                //      (B (L!) (R l k v r)))
                if left.is_leaf() && right.col == Red {
                    let data = right.get_data();
                    // (T cmp 'B l k v r)]
                    return new_node(Black, data.left.clone(), data.key.clone(), data.val.clone(), data.right.clone());
                }

                // Killing a black node with one black child:
                // [(or (B (L!) (and child (B)))
                //      (B (and child (B)) (L!)))
                if left.is_leaf() && right.col == Black {
                    // (black+1 child)
                    let child = right.clone().inc();
                    return child;
                }

                if left.col == Black && right.is_leaf() {
                    // (black+1 child)
                    let child = left.clone().inc();
                    return child;
                }
            }

            if !left.is_leaf() && !right.is_leaf() {
                // ((cons k v) (sorted-map-max l))
                let (k, v) = left.find_max_kvp();
                // (l*         (remove-max l))
                let new_left = remove_max(left);
                // (bubble c l* k v r)
                return bubble(node.col, new_left, k, v, right);
            }

            unreachable!();
        }

        // Kills a double-black, or moves it to the top:
        // (define (bubble c l k v r)
        fn bubble<K: Ord+Clone+Send+Freeze, V: Clone+Send+Freeze>(color: Color, l: NodeRef<K, V>, k: K, v: V, r: NodeRef<K, V>) -> NodeRef<K, V> {
            if l.col == DoubleBlack || r.col == DoubleBlack {
                // (or (double-black? l) (double-black? r))
                // (balance cmp (black+1 c) (black-1 l) k v (black-1 r))
                new_node(color.inc(), l.dec(), k, v, r.dec()).balance()
            } else {
                // [else (T cmp c l k v r)]
                new_node(color, l, k, v, r)
            }
        }

        // Removes the max node:
        fn remove_max<K: Ord+Clone+Send+Freeze, V: Clone+Send+Freeze>(node: NodeRef<K, V>) -> NodeRef<K, V> {
            assert!(!node.is_leaf());
            let (left, key, right) = node.children_and_key();
            if right.is_leaf() {
                // [(T!   l     (L!))  (remove node)]
                remove(node)
            } else {
                //[(T! c l k v r   )  (bubble c l k v (remove-max r))])
                bubble(node.col, left, key.clone(), node.get_data().val.clone(), remove_max(right))
            }
        }

        // Delete the key, and color the new root black:
        // (blacken (del node)))
        del(self.clone(), search_key, removal_count).blacken()
    }
}

#[deriving(Clone)]
struct RedBlackTree<K, V> {
    root: NodeRef<K, V>,
    len: uint,
}

impl<K: Ord+Clone+Send+Freeze, V: Clone+Send+Freeze> RedBlackTree<K, V> {
    pub fn new() -> RedBlackTree<K, V> {
        RedBlackTree {
            root: new_leaf(Black),
            len: 0,
        }
    }

    pub fn find<'a>(&'a self, search_key: &K) -> Option<&'a V> {
        self.root.find(search_key)
    }

    pub fn insert(self, key: K, value: V) -> (RedBlackTree<K, V>, bool) {
        let mut insertion_count = 0xdeadbeaf;
        let new_root = self.root.modify_at(key, value, &mut insertion_count);
        assert!(insertion_count != 0xdeadbeaf);
        (RedBlackTree { root: new_root, len: self.len + insertion_count }, insertion_count != 0)
    }

    pub fn remove(self, key: &K) -> (RedBlackTree<K, V>, bool) {
        let mut removal_count = 0xdeadbeaf;
        let new_root = self.root.delete(key, &mut removal_count);
        assert!(removal_count != 0xdeadbeaf);
        (RedBlackTree { root: new_root, len: self.len - removal_count }, removal_count != 0)
    }

    pub fn len(&self) -> uint {
        self.len
    }

    fn balanced(&self) -> bool {
        self.root.black_balanced()
    }

    fn no_red_red(&self) -> bool {
        self.root.no_red_red()
    }
}

impl<K: Hash+Eq+Send+Freeze+Ord+Clone, V: Send+Freeze+Clone> PersistentMap<K, V> for RedBlackTree<K, V> {
    #[inline]
    fn insert(self, key: K, value: V) -> (RedBlackTree<K, V>, bool) {
        self.insert(key, value)
    }

    #[inline]
    fn remove(self, key: &K) -> (RedBlackTree<K, V>, bool) {
        self.remove(key)
    }
}

impl<K: Hash+Eq+Send+Freeze+Ord+Clone, V: Send+Freeze+Clone> Map<K, V> for RedBlackTree<K, V> {
    #[inline]
    fn find<'a>(&'a self, key: &K) -> Option<&'a V> {
        self.find(key)
    }
}

impl<K: Hash+Eq+Send+Freeze+Ord+Clone, V: Send+Freeze+Clone> Container for RedBlackTree<K, V> {
    #[inline]
    fn len(&self) -> uint {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::RedBlackTree;
    use test::Test;
    use extra::test::BenchHarness;

    #[test]
    fn test_insert_copy() { Test::test_insert(RedBlackTree::<u64, u64>::new()); }

    #[test]
    fn test_insert_overwrite_copy() { Test::test_insert_overwrite(RedBlackTree::<u64, u64>::new()); }

    #[test]
    fn test_remove_copy() { Test::test_remove(RedBlackTree::<u64, u64>::new()); }

    #[test]
    fn test_random_copy() { Test::test_random(RedBlackTree::<u64, u64>::new()); }

    #[bench]
    fn bench_insert_copy_10(bh: &mut BenchHarness) {
        Test::bench_insert(RedBlackTree::<u64, u64>::new(), 10, bh);
    }

    #[bench]
    fn bench_insert_copy_100(bh: &mut BenchHarness) {
        Test::bench_insert(RedBlackTree::<u64, u64>::new(), 100, bh);
    }

    #[bench]
    fn bench_insert_copy_1000(bh: &mut BenchHarness) {
        Test::bench_insert(RedBlackTree::<u64, u64>::new(), 1000, bh);
    }

    #[bench]
    fn bench_insert_copy_50000(bh: &mut BenchHarness) {
        Test::bench_insert(RedBlackTree::<u64, u64>::new(), 50000, bh);
    }

    #[bench]
    fn bench_find_copy_10(bh: &mut BenchHarness) {
        Test::bench_find(RedBlackTree::<u64, u64>::new(), 10, bh);
    }

    #[bench]
    fn bench_find_copy_100(bh: &mut BenchHarness) {
        Test::bench_find(RedBlackTree::<u64, u64>::new(), 100, bh);
    }

    #[bench]
    fn bench_find_copy_1000(bh: &mut BenchHarness) {
        Test::bench_find(RedBlackTree::<u64, u64>::new(), 1000, bh);
    }

    #[bench]
    fn bench_find_copy_50000(bh: &mut BenchHarness) {
        Test::bench_find(RedBlackTree::<u64, u64>::new(), 50000, bh);
    }
}