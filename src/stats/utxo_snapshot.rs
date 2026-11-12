//! Reading Core's `dumptxoutset` snapshot.
//!
//! A snapshot is the only way to answer a **stock** question about the chain:
//! how many coins exist right now, in what script types, holding what value.
//! Every other figure on this site is a **flow**, derived from blocks, and the
//! two are not interchangeable. 371.7 million P2TR outputs have been created;
//! 52.9 million are unspent.
//!
//! # Why this is hand-rolled
//!
//! The snapshot stores every coin in Core's internal compressed form, and
//! `rust-bitcoin` 0.32 does not implement that codec: there is no
//! `decompress_amount` and no compressed-script support. So this module ports
//! two pieces of `src/compressor.cpp`.
//!
//! That is the exception `AGENTS.md` carves out, not a violation of the
//! minimise-dependencies rule: this is a specification with a reference
//! implementation, and the alternative is not using a crate but not reading
//! the file.
//!
//! # Why the result can be trusted
//!
//! Two oracles, both exact, both asserted by
//! `the_snapshot_parse_matches_cores_own_accounting`, which is `#[ignore]`d
//! because it needs the 8.7 GiB file:
//!
//! ```text
//! cargo test --release --features ssr the_snapshot_parse -- --ignored --nocapture
//! ```
//!
//! - the coin count must equal `coins_written` from the `dumptxoutset` result
//! - **the summed value must equal `gettxoutsetinfo <base_height>`'s
//!   `total_amount`**, to the satoshi
//!
//! The second is the one that matters. There is no way to decompress 163
//! million amounts wrongly and still land on the same satoshi, so a match
//! proves the amount codec rather than suggesting it.
//!
//! Measured 2026-11-11 at height 975,464: 163,370,361 coins,
//! 20,110,597.98556717 BTC, both matched.

use std::io::{self, Read};

/// A snapshot's 51-byte header.
#[derive(Debug, Clone)]
pub struct SnapshotHeader {
    pub version: u16,
    pub network_magic: [u8; 4],
    /// Internal byte order, as stored. Reverse for display.
    pub base_blockhash: [u8; 32],
    pub coins_count: u64,
}

impl SnapshotHeader {
    /// Display form, the big-endian hex a block explorer shows.
    pub fn base_hash_hex(&self) -> String {
        self.base_blockhash
            .iter()
            .rev()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// Mainnet's network magic, which the header repeats.
pub const MAINNET_MAGIC: [u8; 4] = [0xf9, 0xbe, 0xb4, 0xd9];

/// The supply cap in satoshis. Nothing in a snapshot can exceed it.
pub const MAX_MONEY: u64 = 21_000_000 * 100_000_000;

/// The only snapshot layout this module knows how to read.
///
/// Checked rather than recorded and ignored. Core has changed this format
/// before, and a future version would be parsed into confident nonsense by a
/// reader that only stores the number.
pub const SUPPORTED_VERSION: u16 = 2;

/// The five-byte prefix every snapshot opens with.
const SNAPSHOT_MAGIC: [u8; 5] = *b"utxo\xff";

/// What an output's script is, for the post-quantum question.
///
/// The distinction that matters is whether a public key is on chain: `P2PK`,
/// `BareMultisig` and `P2TR` publish one when the output is created, the rest
/// publish only a hash or no key at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptKind {
    P2PK,
    P2PKH,
    P2SH,
    P2WPKH,
    P2WSH,
    P2TR,
    BareMultisig,
    /// Pay-to-anchor, `OP_1 <0x4e73>`. A fixed two-byte program with no key.
    P2A,
    /// A witness program of some other version or length.
    OtherWitness,
    Other,
}

impl ScriptKind {
    /// Every variant, so callers can iterate instead of hand-copying a list.
    ///
    /// The compiler cannot check a hand-written array, and three of them
    /// existed: two tests and the binary's label filter. A variant added
    /// without touching all three compiled clean and was silently skipped by
    /// every guard. `ALL` plus `the_variant_list_is_complete` closes that.
    pub const ALL: [ScriptKind; 10] = [
        Self::P2PK,
        Self::P2PKH,
        Self::P2SH,
        Self::P2WPKH,
        Self::P2WSH,
        Self::P2TR,
        Self::BareMultisig,
        Self::P2A,
        Self::OtherWitness,
        Self::Other,
    ];

    /// Whether the output's public key is on chain from creation.
    pub fn is_exposed(self) -> bool {
        matches!(self, Self::P2PK | Self::BareMultisig | Self::P2TR)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::P2PK => "P2PK",
            Self::P2PKH => "P2PKH",
            Self::P2SH => "P2SH",
            Self::P2WPKH => "P2WPKH",
            Self::P2WSH => "P2WSH",
            Self::P2TR => "P2TR",
            Self::BareMultisig => "bare multisig",
            Self::P2A => "P2A",
            Self::OtherWitness => "other witness",
            Self::Other => "other",
        }
    }
}

/// One coin, as the caller sees it.
#[derive(Debug, Clone, Copy)]
pub struct Coin {
    /// Height of the block that created the output.
    pub height: u32,
    pub coinbase: bool,
    pub value_sats: u64,
    pub kind: ScriptKind,
}

/// Core's `DecompressAmount`, from `src/compressor.cpp`.
///
/// The scheme squeezes round numbers into few bytes, which is why a snapshot
/// is 57 bytes per coin rather than the 40-odd a naive encoding would need.
/// Ported rather than approximated: an off-by-one here would still produce
/// plausible-looking totals, which is exactly the failure the value oracle
/// exists to catch.
pub fn decompress_amount(mut x: u64) -> u64 {
    if x == 0 {
        return 0;
    }
    x -= 1;
    let e = x % 10;
    x /= 10;
    let mut n = if e < 9 {
        let d = (x % 9) + 1;
        x /= 9;
        x * 10 + d
    } else {
        x + 1
    };
    for _ in 0..e {
        n = n.saturating_mul(10);
    }
    n
}

/// Core's `CompressAmount`, the inverse of the above.
///
/// Present so the round trip can be tested over a range rather than at a
/// handful of points, and so tests can build snapshot bytes.
///
/// **Defined for real amounts only**, meaning zero to `MAX_MONEY`. Core's C++
/// wraps on overflow because unsigned wrapping is defined there; Rust panics
/// under the overflow checks `cargo test` builds with, so for inputs above
/// roughly 2.05e18 satoshis this is a panic rather than a faithful port.
/// Nothing in a snapshot can reach that: the supply cap is 2.1e15 satoshis,
/// three orders of magnitude below. The guard states the limit rather than
/// pretending the function is total.
pub fn compress_amount(mut n: u64) -> u64 {
    debug_assert!(
        n <= MAX_MONEY,
        "compress_amount is defined for amounts up to MAX_MONEY; {n} is not \
         a real amount and would overflow where Core's C++ wraps"
    );
    if n == 0 {
        return 0;
    }
    let mut e = 0u64;
    while n.is_multiple_of(10) && e < 9 {
        n /= 10;
        e += 1;
    }
    if e < 9 {
        let d = n % 10;
        n /= 10;
        1 + (n * 9 + d - 1) * 10 + e
    } else {
        1 + (n - 1) * 10 + 9
    }
}

/// Classify a decompressed scriptPubKey.
///
/// `special` is the compression tag when Core stored the script in one of its
/// six short forms, which already identifies the type and is authoritative.
/// Everything else arrives as raw bytes and is matched on shape.
pub fn classify(special: Option<u64>, script: &[u8]) -> ScriptKind {
    if let Some(t) = special {
        return match t {
            0 => ScriptKind::P2PKH,
            1 => ScriptKind::P2SH,
            // 2 and 3 are compressed keys, 4 and 5 uncompressed. All P2PK.
            //
            // Enumerated rather than caught by `_`, because `_ => P2PK` made
            // the exposed label the default: a tag Core added later, or a
            // corrupt byte, would silently become a key-bearing output.
            // `Other` is the safe default here, since it understates.
            2..=5 => ScriptKind::P2PK,
            _ => ScriptKind::Other,
        };
    }
    let n = script.len();
    match script {
        [0x51, 0x20, ..] if n == 34 => ScriptKind::P2TR,
        [0x00, 0x14, ..] if n == 22 => ScriptKind::P2WPKH,
        [0x00, 0x20, ..] if n == 34 => ScriptKind::P2WSH,
        [0x51, 0x02, 0x4e, 0x73] => ScriptKind::P2A,
        // **Before the multisig arm, not after.** A witness program is
        // `OP_n <push len> <len bytes>`, and its final byte is program data
        // that can be anything, including 0xAE. Two outputs in the snapshot
        // at height 975,464 are exactly that, a v16 and a v12 program ending
        // 0xAE, and the looser multisig shape claimed both and published 843
        // satoshis as key-bearing when neither carries a key.
        //
        // The two shapes cannot collide the other way: a bare multisig's
        // second byte is a pubkey push, 0x21 or 0x41, which never equals
        // `n - 2` for any real key count.
        [first, len, ..]
            if n >= 4
                && (0x51..=0x60).contains(first)
                && *len as usize == n - 2 =>
        {
            ScriptKind::OtherWitness
        }
        // OP_m <key>... OP_n OP_CHECKMULTISIG, keys in the output script.
        //
        // **The pushes have to be key-shaped**, not merely present. Matching
        // on the first and last byte alone admitted 45 outputs in the
        // snapshot at height 975,464 that are data wearing a multisig shape:
        // an RSKBLOCK merged-mining commitment, a text/plain payload, runs
        // of 0xff. None carries a key, and bare multisig counts as exposed,
        // so each was published as key-bearing.
        [first, ..]
            if n > 0
                && script[n - 1] == 0xAE
                && (0x51..=0x60).contains(first)
                && has_key_shaped_pushes(script) =>
        {
            ScriptKind::BareMultisig
        }
        _ => ScriptKind::Other,
    }
}

/// Whether a multisig-shaped script's pushes are all 33 or 65 bytes.
///
/// A public key is one length or the other, so a push of any other size means
/// the script carries data rather than keys. Walks between the leading
/// `OP_m` and the trailing `OP_n OP_CHECKMULTISIG` and requires the pushes to
/// land exactly on that boundary: a script that overruns is malformed and
/// not a multisig either.
fn has_key_shaped_pushes(script: &[u8]) -> bool {
    let end = script.len() - 2; // before OP_n OP_CHECKMULTISIG
    let mut i = 1; // after OP_m
    let mut keys = 0;
    while i < end {
        let push = script[i] as usize;
        if push != 33 && push != 65 {
            return false;
        }
        i += 1 + push;
        keys += 1;
    }
    i == end && keys > 0
}

/// A byte source that refills, so an 8.7 GiB file is read without holding it.
struct Cursor<R: Read> {
    inner: R,
    buf: Vec<u8>,
    pos: usize,
}

/// Enough slack that any single record fits without a refill mid-field.
/// The largest is a raw script, which consensus bounds well below this.
const SLACK: usize = 1 << 17;

impl<R: Read> Cursor<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            buf: Vec::new(),
            pos: 0,
        }
    }

    fn fill(&mut self, need: usize) -> io::Result<()> {
        if self.pos + need <= self.buf.len() {
            return Ok(());
        }
        self.buf.drain(..self.pos);
        self.pos = 0;
        let mut chunk = vec![0u8; 1 << 22];
        while self.buf.len() < need + SLACK {
            let n = self.inner.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
        if self.buf.len() < need {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "snapshot ended mid-record",
            ));
        }
        Ok(())
    }

    fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        self.fill(n)?;
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }

    /// Bitcoin's CompactSize, used for counts and output indices.
    fn compact(&mut self) -> io::Result<u64> {
        let c = self.u8()?;
        Ok(match c {
            0..=252 => u64::from(c),
            253 => {
                u64::from(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
            }
            254 => {
                u64::from(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
            }
            _ => u64::from_le_bytes(self.take(8)?.try_into().unwrap()),
        })
    }

    /// Core's `VARINT`, which is **not** CompactSize: base-128, most
    /// significant group first, with each continuation adding one so that
    /// every integer has exactly one encoding.
    fn varint(&mut self) -> io::Result<u64> {
        let mut n: u64 = 0;
        loop {
            let c = self.u8()?;
            n = (n << 7) | u64::from(c & 0x7F);
            if c & 0x80 != 0 {
                n = n.checked_add(1).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "varint overflow",
                    )
                })?;
            } else {
                return Ok(n);
            }
        }
    }
}

/// Read the header and then every coin, handing each to `on_coin`.
///
/// Streaming rather than collecting: 163 million coins will not fit in memory
/// on the 4 GB droplet, and the caller only ever wants aggregates.
pub fn parse<R: Read>(
    reader: R,
    mut on_coin: impl FnMut(Coin),
) -> io::Result<SnapshotHeader> {
    let mut c = Cursor::new(reader);

    let magic: [u8; 5] = c.take(5)?.try_into().unwrap();
    if magic != SNAPSHOT_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("not a utxo snapshot: magic {magic:02x?}"),
        ));
    }
    let version = u16::from_le_bytes(c.take(2)?.try_into().unwrap());
    if version != SUPPORTED_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "snapshot version {version}, this reader understands only \
                 {SUPPORTED_VERSION}; the record layout may have changed"
            ),
        ));
    }
    let network_magic: [u8; 4] = c.take(4)?.try_into().unwrap();
    if network_magic != MAINNET_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "network magic {network_magic:02x?} is not mainnet; every \
                 figure derived from this would be about another chain"
            ),
        ));
    }
    let base_blockhash: [u8; 32] = c.take(32)?.try_into().unwrap();
    let coins_count = u64::from_le_bytes(c.take(8)?.try_into().unwrap());

    let header = SnapshotHeader {
        version,
        network_magic,
        base_blockhash,
        coins_count,
    };

    let mut seen: u64 = 0;
    while seen < coins_count {
        // One group per transaction: its txid, then its unspent outputs.
        let _txid = c.take(32)?;
        let outputs = c.compact()?;
        for _ in 0..outputs {
            let _vout = c.compact()?;
            let code = c.varint()?;
            let height = (code >> 1) as u32;
            let coinbase = code & 1 == 1;
            let value_sats = decompress_amount(c.varint()?);

            let size = c.varint()?;
            let kind = if size < 6 {
                // A special form: 20 bytes for a hash, 32 for a key.
                let n = if size < 2 { 20 } else { 32 };
                c.take(n)?;
                classify(Some(size), &[])
            } else {
                let n = (size - 6) as usize;
                let script = c.take(n)?.to_vec();
                classify(None, &script)
            };

            on_coin(Coin {
                height,
                coinbase,
                value_sats,
                kind,
            });
            seen += 1;
        }
    }
    Ok(header)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-derived from Core's algorithm, worked through in the comments so
    /// a reader can check them without running anything.
    ///
    /// **These prove the two functions agree with the algorithm as written,
    /// not that the algorithm matches Core.** Only the file oracle does that,
    /// by summing 163 million real amounts and landing on Core's own total.
    /// A first draft of this test asserted `(6, 50_000_000)` from memory and
    /// was simply wrong, which is the argument for deriving rather than
    /// recalling.
    #[test]
    fn amount_vectors_derived_by_hand() {
        for (compressed, sats) in [
            (0u64, 0u64),
            // one satoshi: no trailing zeros, e = 0, d = 1, so 1 + 0 + 0
            (1, 1),
            // 1 BTC = 1e8: eight zeros strip to mantissa 1, e = 8,
            // 1 + (0 * 9 + 1 - 1) * 10 + 8
            (9, 100_000_000),
            // 0.5 BTC = 5e7: seven zeros strip to mantissa 5, e = 7,
            // 1 + (0 * 9 + 5 - 1) * 10 + 7
            (48, 50_000_000),
            // 50 BTC, the original subsidy: nine trailing zeros, so the
            // exponent loop stops at its e < 9 bound and the OTHER branch
            // runs: 1 + (5 - 1) * 10 + 9. Getting this one wrong twice is
            // why the vectors are derived here rather than recalled.
            (50, 5_000_000_000),
        ] {
            assert_eq!(
                decompress_amount(compressed),
                sats,
                "decompress({compressed})"
            );
            assert_eq!(compress_amount(sats), compressed, "compress({sats})");
        }
    }

    /// Round trip across the range a real coin can take, which is zero to
    /// `MAX_MONEY` and no further.
    ///
    /// Point vectors alone would not catch a codec that is right for round
    /// numbers and wrong for the awkward ones, which is most of a UTXO set.
    ///
    /// **Named for its domain rather than "every".** An earlier version swept
    /// past `MAX_MONEY` into the range where `compress_amount` would overflow,
    /// which is not a range any coin occupies, so the extra coverage was
    /// imaginary.
    #[test]
    fn amounts_up_to_the_supply_cap_round_trip() {
        let mut cases: Vec<u64> = vec![
            0,
            1,
            546,
            5_000,
            100_000_000,
            2_100_000_000_000_000,
            21_000_000 * 100_000_000,
        ];
        // Deterministic spread, no RNG: powers of ten and their neighbours
        // are where a base-ten scheme goes wrong.
        let mut p = 1u64;
        for _ in 0..16 {
            cases.extend([p, p + 1, p.saturating_sub(1), p * 3 + 7]);
            p = p.saturating_mul(10);
        }
        cases.retain(|n| *n <= MAX_MONEY);
        assert!(
            cases.len() > 40,
            "the sweep collapsed to {} cases",
            cases.len()
        );
        for n in cases {
            assert_eq!(
                decompress_amount(compress_amount(n)),
                n,
                "round trip failed for {n} sats"
            );
        }
    }

    /// The exposed set is exactly the three key-in-output types.
    ///
    /// Iterates `ALL` rather than a list written here, so a new variant
    /// joins this check by existing rather than by being remembered.
    #[test]
    fn only_key_bearing_outputs_are_exposed() {
        use ScriptKind::*;
        let exposed: Vec<&str> = ScriptKind::ALL
            .iter()
            .filter(|k| k.is_exposed())
            .map(|k| k.label())
            .collect();
        assert_eq!(
            exposed,
            vec!["P2PK", "P2TR", "bare multisig"]
                .into_iter()
                .filter(|l| exposed.contains(l))
                .collect::<Vec<_>>(),
            "the exposed set changed; it must be exactly the three types \
             that put a key in the output"
        );
        for k in [P2PK, BareMultisig, P2TR] {
            assert!(k.is_exposed(), "{} must be exposed", k.label());
        }
        for k in ScriptKind::ALL
            .iter()
            .filter(|k| !matches!(k, P2PK | BareMultisig | P2TR))
        {
            assert!(!k.is_exposed(), "{} must not be exposed", k.label());
        }
    }

    /// `ALL` must hold every variant, or every guard that iterates it is
    /// quietly narrower than it looks.
    ///
    /// The match is exhaustive, so adding a variant without adding it here
    /// fails to compile rather than passing silently.
    #[test]
    fn the_variant_list_is_complete() {
        use ScriptKind::*;
        for k in ScriptKind::ALL {
            // Exhaustive by construction: a new variant breaks the build.
            let _: () = match k {
                P2PK | P2PKH | P2SH | P2WPKH | P2WSH | P2TR | BareMultisig
                | P2A | OtherWitness | Other => (),
            };
        }
        let mut labels: Vec<&str> =
            ScriptKind::ALL.iter().map(|k| k.label()).collect();
        let n = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), n, "two variants share a label");
        assert_eq!(n, 10, "ALL has {n} entries; add the new variant here");
    }

    /// Shape matching, including the two that are easy to confuse.
    ///
    /// Pay-to-anchor and Taproot are both witness v1 and both start `0x51`.
    /// They differ only in program length, and calling an anchor "Taproot"
    /// would move six million keyless outputs into the exposed set.
    #[test]
    fn scripts_are_classified_by_shape() {
        let p2tr = [&[0x51u8, 0x20][..], &[0xABu8; 32][..]].concat();
        assert_eq!(classify(None, &p2tr), ScriptKind::P2TR);

        assert_eq!(
            classify(None, &[0x51, 0x02, 0x4e, 0x73]),
            ScriptKind::P2A,
            "an anchor is witness v1 like Taproot but carries no key"
        );

        let p2wpkh = [&[0x00u8, 0x14][..], &[0xCDu8; 20][..]].concat();
        assert_eq!(classify(None, &p2wpkh), ScriptKind::P2WPKH);

        let p2wsh = [&[0x00u8, 0x20][..], &[0xCDu8; 32][..]].concat();
        assert_eq!(classify(None, &p2wsh), ScriptKind::P2WSH);

        // OP_1 <33-byte key> <33-byte key> OP_2 OP_CHECKMULTISIG.
        // The pushes must be key-shaped: a run of filler bytes between the
        // opcodes is data in a multisig costume, and 45 such outputs exist
        // in the real snapshot.
        let key = |b: u8| [&[33u8][..], &[b; 33][..]].concat();
        let ms =
            [&[0x51u8][..], &key(0xA1), &key(0xA2), &[0x52, 0xAE][..]].concat();
        assert_eq!(classify(None, &ms), ScriptKind::BareMultisig);

        // Uncompressed keys are 65 bytes and just as valid. A check that
        // accepts only 33 would drop every pre-2012 multisig.
        let key65 = [&[65u8][..], &[0xB1u8; 65][..]].concat();
        let ms65 = [&[0x51u8][..], &key65, &[0x51, 0xAE][..]].concat();
        assert_eq!(classify(None, &ms65), ScriptKind::BareMultisig);

        // Same opcodes, pushes that are not keys. This is the RSKBLOCK and
        // text/plain shape, and it must not count as exposed.
        let fake = [&[0x52u8][..], &[0x00u8; 60][..], &[0xAE][..]].concat();
        assert_ne!(
            classify(None, &fake),
            ScriptKind::BareMultisig,
            "data wearing a multisig shape carries no key"
        );
        assert!(!classify(None, &fake).is_exposed());

        // Multisig ends OP_CHECKMULTISIG. Ending OP_CHECKSIG instead makes
        // it some other script, and it must not be claimed. The terminator
        // is 0xAC rather than 0x00 on purpose: zero is what a careless
        // mutation of the 0xAE check compares against, so a zero-terminated
        // fixture cannot tell that check is gone, and 0xAC is what a real
        // script ends with anyway.
        let no_checkmultisig =
            [&[0x51u8][..], &key(0xA1), &key(0xA2), &[0x52, 0xAC][..]].concat();
        assert_ne!(
            classify(None, &no_checkmultisig),
            ScriptKind::BareMultisig,
            "a script not ending OP_CHECKMULTISIG is not bare multisig"
        );

        // **A real witness program that ends in 0xAE**, taken from the
        // snapshot at height 975,464: witness v16 with a 21-byte program.
        // The multisig arm matches on first byte in OP_1..OP_16 and last
        // byte OP_CHECKMULTISIG, which this satisfies by coincidence, so
        // before the arms were reordered this output and one other were
        // published as key-bearing when neither carries a key.
        let witness_ending_ae = [
            0x60u8, 0x15, 0x00, 0x4f, 0x31, 0xcf, 0xda, 0xeb, 0xbd, 0xdd, 0x8f,
            0xe4, 0xd2, 0x6b, 0xa1, 0xf2, 0xf7, 0x5d, 0x42, 0x36, 0xb2, 0x2d,
            0xae,
        ];
        assert_eq!(
            classify(None, &witness_ending_ae),
            ScriptKind::OtherWitness,
            "a witness program whose last byte is 0xAE must not be claimed \
             by the multisig arm, which would make it count as exposed"
        );
        assert!(
            !classify(None, &witness_ending_ae).is_exposed(),
            "no key in a witness program, so it cannot be exposed"
        );

        // The compression tag is authoritative where Core supplied one.
        assert_eq!(classify(Some(0), &[]), ScriptKind::P2PKH);
        assert_eq!(classify(Some(1), &[]), ScriptKind::P2SH);
        for t in 2..=5 {
            assert_eq!(classify(Some(t), &[]), ScriptKind::P2PK);
        }
        // A tag Core has not defined must not inherit the exposed label.
        // `_ => P2PK` made key-bearing the default for anything unknown,
        // so a future Core tag or a corrupt byte would have invented
        // exposure that is not there.
        for t in [6u64, 7, 99] {
            assert_eq!(
                classify(Some(t), &[]),
                ScriptKind::Other,
                "compression tag {t} is not one Core defines, so it cannot \
                 be assumed to carry a key"
            );
            assert!(!classify(Some(t), &[]).is_exposed());
        }
    }

    /// Build a snapshot in memory, so the record walk is exercised without
    /// an 8.7 GiB file.
    ///
    /// Nothing drove `parse()` past the magic check until this existed. Four
    /// separate mutations to `varint`, `compact`, the record layout and the
    /// header width all survived the suite, because no test ever read a
    /// single valid record.
    fn snapshot(coins: &[(u32, bool, u64, &[u8], u64)]) -> Vec<u8> {
        fn varint(mut n: u64) -> Vec<u8> {
            let mut tmp = vec![(n & 0x7F) as u8];
            while n > 0x7F {
                n = (n >> 7) - 1;
                tmp.push(((n & 0x7F) as u8) | 0x80);
            }
            tmp.reverse();
            tmp
        }
        fn compact(n: u64) -> Vec<u8> {
            if n < 253 {
                vec![n as u8]
            } else {
                let mut v = vec![253u8];
                v.extend_from_slice(&(n as u16).to_le_bytes());
                v
            }
        }
        let mut v = Vec::new();
        v.extend_from_slice(b"utxo\xff");
        v.extend_from_slice(&2u16.to_le_bytes());
        v.extend_from_slice(&MAINNET_MAGIC);
        v.extend_from_slice(&[0x11u8; 32]);
        v.extend_from_slice(&(coins.len() as u64).to_le_bytes());
        // One group per coin keeps the fixture readable; the walk is the same.
        for (i, (height, coinbase, amount, script, vout)) in
            coins.iter().enumerate()
        {
            v.extend_from_slice(&[i as u8; 32]); // txid
            v.extend(compact(1)); // one output in this group
            v.extend(compact(*vout));
            v.extend(varint(u64::from(*height) * 2 + u64::from(*coinbase)));
            v.extend(varint(compress_amount(*amount)));
            v.extend(varint(script.len() as u64 + 6));
            v.extend_from_slice(script);
        }
        v
    }

    /// Every field of a record is read back exactly as written.
    ///
    /// Heights and amounts are chosen to need multi-byte varints, because a
    /// single-byte fixture cannot tell Core's VARINT from CompactSize and
    /// the two disagree from 128 upwards.
    #[test]
    fn a_record_round_trips_through_the_parser() {
        let p2tr = [&[0x51u8, 0x20][..], &[0x77u8; 32][..]].concat();
        // **At least one vout above 127.** CompactSize and Core's VARINT
        // agree below 128 and diverge above it, so a fixture of vout-zero
        // records cannot tell the two decoders apart: swapping them survived
        // the whole suite until this value changed.
        let coins: Vec<(u32, bool, u64, &[u8], u64)> = vec![
            (1, true, 5_000_000_000, &p2tr, 0),
            (975_464, false, 1, &[0x6au8, 0x01, 0x02], 200),
            (210_000, false, 2_100_000_000_000, &p2tr, 5),
        ];
        let bytes = snapshot(&coins);

        let mut got = Vec::new();
        let header = parse(&bytes[..], |c| got.push(c)).expect("parses");

        assert_eq!(header.version, 2);
        assert_eq!(header.coins_count, 3);
        assert_eq!(got.len(), 3, "every coin must come back");
        for (i, (h, cb, amt, _, _)) in coins.iter().enumerate() {
            assert_eq!(got[i].height, *h, "height of coin {i}");
            assert_eq!(got[i].coinbase, *cb, "coinbase flag of coin {i}");
            assert_eq!(got[i].value_sats, *amt, "amount of coin {i}");
        }
        assert_eq!(got[0].kind, ScriptKind::P2TR);
        assert_eq!(got[2].kind, ScriptKind::P2TR);
    }

    /// A truncated file is an error, not a short answer.
    #[test]
    fn a_truncated_snapshot_is_refused() {
        let p2tr = [&[0x51u8, 0x20][..], &[0x77u8; 32][..]].concat();
        let full =
            snapshot(&[(1, true, 50, &p2tr, 0), (2, false, 60, &p2tr, 200)]);
        let cut = &full[..full.len() - 10];
        let err = parse(cut, |_| {}).expect_err("truncation must fail");
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    /// The real file, against Core's own accounting. The only check that
    /// proves the codec rather than its internal consistency.
    ///
    /// `#[ignore]`d because it needs the snapshot, which is 8.7 GiB and not
    /// in the repository. Path from `UTXO_SNAPSHOT`, defaulting to where the
    /// 2026-11-11 dump was put:
    ///
    /// ```text
    /// cargo test --release --features ssr the_snapshot_parse -- --ignored --nocapture
    /// ```
    ///
    /// The expected total came from `gettxoutsetinfo none 975464`, which
    /// reports the set at the snapshot's own height rather than at the tip.
    #[test]
    #[ignore]
    fn the_snapshot_parse_matches_cores_own_accounting() {
        const EXPECTED_COINS: u64 = 163_370_361;
        const EXPECTED_SATS: u128 = 2_011_059_798_556_717;
        const EXPECTED_HASH: &str =
            "0000000000000000000138036994e1733914b51591f3ad91a8e6b3d349fa7dec";

        let path = std::env::var("UTXO_SNAPSHOT").unwrap_or_else(|_| {
            format!(
                "{}/opt/utxo-20261111.dat",
                std::env::var("HOME").unwrap_or_default()
            )
        });
        let file = std::fs::File::open(&path)
            .unwrap_or_else(|e| panic!("{path}: {e}"));
        let reader = std::io::BufReader::with_capacity(1 << 22, file);

        let (mut coins, mut sats) = (0u64, 0u128);
        let header = parse(reader, |c| {
            coins += 1;
            sats += u128::from(c.value_sats);
        })
        .expect("parses");

        assert_eq!(header.base_hash_hex(), EXPECTED_HASH, "wrong snapshot");
        assert_eq!(coins, header.coins_count, "parsed count vs header");
        assert_eq!(coins, EXPECTED_COINS, "parsed count vs dumptxoutset");
        assert_eq!(
            sats, EXPECTED_SATS,
            "parsed {} sats against gettxoutsetinfo's {EXPECTED_SATS}; a \
             difference here means the amount codec is wrong, because 163 \
             million amounts cannot be decompressed wrongly and still sum \
             to the same satoshi",
            sats
        );
    }

    /// The supply cap, which bounds `compress_amount`'s domain.
    ///
    /// Asserted directly because the round-trip test only filters by it: a
    /// wrong constant there just means fewer cases, which nothing notices.
    #[test]
    fn the_supply_cap_is_twenty_one_million_coins() {
        assert_eq!(MAX_MONEY, 2_100_000_000_000_000);
        assert_eq!(MAX_MONEY / 100_000_000, 21_000_000);
    }

    /// A snapshot from another chain, or a layout this reader does not know,
    /// must be refused rather than parsed into figures about the wrong thing.
    #[test]
    fn a_foreign_chain_or_version_is_refused() {
        let p2tr = [&[0x51u8, 0x20][..], &[0x77u8; 32][..]].concat();
        let good = snapshot(&[(1, true, 50, &p2tr, 0)]);
        parse(&good[..], |_| {}).expect("the unmodified fixture parses");

        // Version lives at bytes 5..7.
        let mut wrong_version = good.clone();
        wrong_version[5] = 3;
        let err = parse(&wrong_version[..], |_| {})
            .expect_err("version 3 must be refused");
        assert!(
            format!("{err}").contains("version"),
            "the error must name the version: {err}"
        );

        // Network magic lives at bytes 7..11. This is testnet's.
        let mut wrong_chain = good.clone();
        wrong_chain[7..11].copy_from_slice(&[0x0b, 0x11, 0x09, 0x07]);
        let err = parse(&wrong_chain[..], |_| {})
            .expect_err("a testnet snapshot must be refused");
        assert!(
            format!("{err}").contains("mainnet"),
            "the error must name the chain: {err}"
        );
    }

    /// A header that is not a snapshot must be refused rather than parsed
    /// into nonsense.
    #[test]
    fn a_foreign_file_is_rejected() {
        let bogus = [0u8; 128];
        let err = parse(&bogus[..], |_| {}).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
