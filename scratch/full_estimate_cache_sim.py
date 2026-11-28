"""
Cache-hit simulation for evaluating all k-subsets of an n-letter alphabet,
where each k-subset needs the results of all of its (k-1)-subsets.

Each cached (k-1)-subset has a size in bytes (its node chunk on disk), and the
cache capacity is a byte budget. Capacities are given in units of the mean
item size, so `--sizes 1000` means "room for ~1000 average items".

Usage:
    python3 cache_sim.py                      # n=26, k=5, letter size model
    python3 cache_sim.py --n 20 --k 4 --sizes 100 500 2000 --size-model iid

Size models:
    unit    every item has size 1 (the original equal-size simulation)
    iid     independent lognormal sizes
    letter  lognormal, log-size = sum of per-letter weights + noise, so subsets
            sharing letters have similar sizes (like rare letters surviving
            pruning more often)

Subsets are represented as bitmasks (bit i set = letter i present).
"""

import argparse
import heapq
import itertools
import random
from collections import OrderedDict
from math import comb, exp, sqrt

NEVER = float("inf")


def eval_order(n, k, order):
    sets = list(itertools.combinations(range(n), k))  # lexicographic
    if order == "colex":
        sets.sort(key=lambda s: s[::-1])
    return sets


def lookup_sequence(sets):
    """The (k-1)-subset lookups, in the order they happen."""
    seq = []
    for s in sets:
        mask = 0
        for x in s:
            mask |= 1 << x
        for x in s:
            seq.append(mask & ~(1 << x))  # drop one letter
    return seq


def next_uses(seq):
    """next_use[i] = index of the next lookup of seq[i], or NEVER."""
    next_use = [NEVER] * len(seq)
    last_seen = {}
    for i in range(len(seq) - 1, -1, -1):
        next_use[i] = last_seen.get(seq[i], NEVER)
        last_seen[seq[i]] = i
    return next_use


def gen_sizes(n, k, model, sigma, seed):
    rng = random.Random(seed)
    masks = [sum(1 << x for x in s) for s in itertools.combinations(range(n), k - 1)]
    if model == "unit":
        return {m: 1.0 for m in masks}
    if model == "iid":
        return {m: exp(rng.gauss(0, sigma)) for m in masks}
    # letter: 3/4 of the log-variance comes from shared per-letter weights.
    w = [rng.gauss(0, sigma * sqrt(0.75 / (k - 1))) for _ in range(n)]
    sizes = {}
    for m in masks:
        log_size = sum(w[x] for x in range(n) if m >> x & 1)
        sizes[m] = exp(log_size + rng.gauss(0, sigma * 0.5))
    return sizes


class Stats:
    def __init__(self):
        self.misses = 0
        self.miss_bytes = 0.0

    def miss(self, size):
        self.misses += 1
        self.miss_bytes += size


def unlimited(seq, size):
    """Unbounded cache: every subset misses exactly once."""
    seen = set()
    st = Stats()
    for t in seq:
        if t not in seen:
            st.miss(size[t])
            seen.add(t)
    return st


def lru(seq, size, capacity):
    cache = OrderedDict()
    used = 0.0
    st = Stats()
    for t in seq:
        if t in cache:
            cache.move_to_end(t)
            continue
        st.miss(size[t])
        if size[t] > capacity:
            continue
        while used + size[t] > capacity:
            _, s = cache.popitem(last=False)  # evict least recently used
            used -= s
        cache[t] = size[t]
        used += size[t]
    return st


def gds(seq, size, capacity):
    """GreedyDual-Size with cost 1: online, prefers keeping small items.

    Each item has priority H = L + 1/size, refreshed on every hit. Evicting
    the min-H item raises the floor L to its H, so idle items age out.
    """
    cache = {}  # item -> current H
    heap = []  # (H, item); lazy deletion of stale entries
    floor = 0.0
    used = 0.0
    st = Stats()
    for t in seq:
        if t in cache:
            cache[t] = floor + 1 / size[t]
            heapq.heappush(heap, (cache[t], t))
            continue
        st.miss(size[t])
        if size[t] > capacity:
            continue
        while used + size[t] > capacity:
            h, victim = heapq.heappop(heap)
            if cache.get(victim) != h:
                continue
            floor = h
            del cache[victim]
            used -= size[victim]
        cache[t] = floor + 1 / size[t]
        heapq.heappush(heap, (cache[t], t))
        used += size[t]
    return st


def belady(seq, next_use, size, capacity):
    """Furthest-next-use eviction with bypass (MIN when sizes are equal)."""
    cache = {}  # item -> its current next-use time
    heap = []  # (-next_use, item); lazy deletion of stale entries
    used = 0.0
    st = Stats()
    for i, t in enumerate(seq):
        nu = next_use[i]
        if t in cache:
            cache[t] = nu
            heapq.heappush(heap, (-nu, t))
            continue

        st.miss(size[t])
        if nu == NEVER or size[t] > capacity:
            continue  # never needed again (or can't fit): don't store it

        # Collect victims used further in the future than t until t fits.
        victims = []
        freed = 0.0
        bypass = False
        while used - freed + size[t] > capacity:
            neg, victim = heapq.heappop(heap)
            if cache.get(victim) != -neg:
                continue
            victims.append((neg, victim))
            freed += size[victim]
            if -neg <= nu:
                bypass = True  # t is needed later than a victim: keep them
                break
        if bypass:
            for v in victims:
                heapq.heappush(heap, v)
            continue
        for _, victim in victims:
            del cache[victim]
        used -= freed

        cache[t] = nu
        heapq.heappush(heap, (-nu, t))
        used += size[t]
    return st


def belady_size(seq, next_use, size, capacity, samples, seed):
    """Sampled Belady-Size: evict max size * (next_use - now), with bypass.

    That score is the byte-time an item will occupy per miss it saves. Its
    ordering shifts as `now` advances, so instead of a heap we score
    `samples` random cached items and evict the worst, as Redis/LRB do.
    """
    rng = random.Random(seed)
    nu_of = {}  # item -> next-use time
    slot = {}  # item -> index in items, for O(1) swap-remove
    items = []
    used = 0.0
    st = Stats()

    def remove(x):
        j = slot.pop(x)
        last = items.pop()
        if last != x:
            items[j] = last
            slot[last] = j
        del nu_of[x]

    for i, t in enumerate(seq):
        nu = next_use[i]
        if t in nu_of:
            nu_of[t] = nu
            continue

        st.miss(size[t])
        if nu == NEVER or size[t] > capacity:
            continue

        incoming = size[t] * (nu - i)
        victims = []
        freed = 0.0
        bypass = False
        while used - freed + size[t] > capacity:
            cands = (
                items
                if len(items) <= samples
                else [items[rng.randrange(len(items))] for _ in range(samples)]
            )
            victim = max(
                (c for c in cands if c not in victims),
                key=lambda c: size[c] * (nu_of[c] - i),
                default=None,
            )
            if victim is None or size[victim] * (nu_of[victim] - i) <= incoming:
                bypass = True
                break
            victims.append(victim)
            freed += size[victim]
        if bypass:
            continue
        for victim in victims:
            remove(victim)
        used -= freed

        nu_of[t] = nu
        slot[t] = len(items)
        items.append(t)
        used += size[t]
    return st


def check_lex_proof(n, k):
    """In lex order, every k-set without letter 0 ('A') should get k/k hits."""
    seen = set()
    no_a = perfect = 0
    for s in itertools.combinations(range(n), k):
        hits = 0
        for t in itertools.combinations(s, k - 1):
            if t in seen:
                hits += 1
            else:
                seen.add(t)
        if 0 not in s:
            no_a += 1
            perfect += hits == k
    return no_a, perfect


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--n", type=int, default=26)
    p.add_argument("--k", type=int, default=10)
    p.add_argument(
        "--sizes", type=int, nargs="+", default=[500, 1500, 3000, 6000, 10000]
    )
    p.add_argument(
        "--orders", nargs="+", default=["lex", "colex"], choices=["lex", "colex"]
    )
    p.add_argument("--size-model", default="letter", choices=["unit", "iid", "letter"])
    p.add_argument("--sigma", type=float, default=1.0, help="log-size std dev")
    p.add_argument("--samples", type=int, default=32, help="Belady-Size sample")
    p.add_argument("--seed", type=int, default=0)
    a = p.parse_args()

    size = gen_sizes(a.n, a.k, a.size_model, a.sigma, a.seed)
    mean = sum(size.values()) / len(size)
    ss = sorted(size.values())
    total = comb(a.n, a.k) * a.k
    distinct = comb(a.n, a.k - 1)
    print(
        f"n={a.n} k={a.k}: {comb(a.n, a.k):,} sets, {total:,} lookups, "
        f"{distinct:,} distinct ({a.k-1})-subsets"
    )
    print(
        f"sizes ({a.size_model}, sigma={a.sigma}): mean {mean:.2f}, "
        f"p10 {ss[len(ss) // 10] / mean:.2f}x, p90 {ss[9 * len(ss) // 10] / mean:.2f}x"
        f", max {ss[-1] / mean:.1f}x mean"
    )
    # Every subset is looked up n-k+1 times, so the byte max equals this.
    print(f"Theoretical max hit rate: {1 - distinct / total:.2%}\n")

    no_a, perfect = check_lex_proof(a.n, a.k)
    print(
        f"Lex proof check: {perfect:,} of {no_a:,} sets without 'A' "
        f"got {a.k}/{a.k} hits\n"
    )

    for order in a.orders:
        seq = lookup_sequence(eval_order(a.n, a.k, order))
        nxt = next_uses(seq)
        total_bytes = sum(size[t] for t in seq)
        u = unlimited(seq, size)
        print(f"[{order}] unlimited cache: hit rate {1 - u.misses / total:.2%}")
        print(f"[{order}] hit rate (objects / bytes):")
        for c in a.sizes:
            cap = c * mean
            runs = [
                ("LRU", lru(seq, size, cap)),
                ("GDS", gds(seq, size, cap)),
                ("Belady", belady(seq, nxt, size, cap)),
                (
                    "Belady-Size",
                    belady_size(seq, nxt, size, cap, a.samples, a.seed),
                ),
            ]
            cols = "  ".join(
                f"{name} {1 - st.misses / total:6.2%}/"
                f"{1 - st.miss_bytes / total_bytes:6.2%}"
                for name, st in runs
            )
            print(f"  cache={c:>6,}  {cols}")
        print()


if __name__ == "__main__":
    main()
