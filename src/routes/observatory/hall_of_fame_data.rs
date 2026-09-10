//! Curated Hall of Fame entries, verified against the live database and
//! cross-referenced historical sources. Every block height and stat has been
//! confirmed via direct DB query or authoritative documentation.

use crate::stats::types::{HallOfFameEntry, HofCategory};

use HofCategory::*;

pub const HALL_OF_FAME: &[HallOfFameEntry] = &[
    // =====================================================================
    // MILESTONES
    // =====================================================================
    HallOfFameEntry {
        slug: "genesis-block",
        title: "Genesis Block",
        description: "Block 0, the first Bitcoin block ever mined. The coinbase contains the famous headline: \"The Times 03/Jan/2009 Chancellor on brink of second bailout for banks.\" The 50 BTC reward is unspendable due to a quirk in the original code.",
        short_context: Some("Block 0 contains the famous headline: \"The Times 03/Jan/2009 Chancellor on brink of second bailout for banks\""),
        category: Milestones,
        date: "2009-01-03",
        block: Some(0),
        txid: None,
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "first-transaction",
        title: "First Bitcoin Transaction",
        description: "Satoshi Nakamoto sent 10 BTC to Hal Finney in block 170, the first person-to-person Bitcoin transaction. Finney reportedly ran Bitcoin on his laptop and was the first person other than Satoshi to mine blocks.",
        short_context: Some("Satoshi sent 10 BTC to Hal Finney in block 170. Finney reportedly ran Bitcoin on his laptop while fighting ALS"),
        category: Milestones,
        date: "2009-01-12",
        block: Some(170),
        txid: Some("f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16"),
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "pizza-day",
        title: "Bitcoin Pizza Day",
        description: "Laszlo Hanyecz paid 10,000 BTC for two Papa John's pizzas, the first known real-world Bitcoin purchase. Jeremy Sturdivant accepted the deal. Those coins would be worth billions today.",
        short_context: Some("Laszlo Hanyecz paid 10,000 BTC for two Papa John's pizzas, the first known real-world Bitcoin purchase"),
        category: Milestones,
        date: "2010-05-22",
        block: Some(57043),
        txid: Some("a1075db55d416d3ca199f55b6084e2115b9345e16c5cf302fc80e9d5fbf5d48d"),
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "mt-gox-opens",
        title: "Mt. Gox Exchange Opens",
        description: "Originally a Magic: The Gathering card trading site, Mt. Gox launched as a Bitcoin exchange. It would grow to handle 70% of all BTC trades before its catastrophic collapse in 2014.",
        short_context: Some("Originally a Magic: The Gathering card trading site. Would grow to handle 70% of all BTC trades before its collapse"),
        category: Milestones,
        date: "2010-07-18",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "satoshi-last-post",
        title: "Satoshi's Last Public Post",
        description: "Satoshi's final message on BitcoinTalk discussed DoS attack prevention. After this post, Satoshi was never heard from publicly again. The last known login was December 13, 2010.",
        short_context: Some("Satoshi's final message on BitcoinTalk discussed DoS attack prevention. He was never heard from publicly again"),
        category: Milestones,
        date: "2010-12-12",
        block: None,
        txid: None,
        highlight: true,
        source: Some(("BitcoinTalk Post", "https://bitcointalk.org/index.php?topic=2228.msg29479#msg29479")),
    },
    HallOfFameEntry {
        slug: "btc-reaches-1",
        title: "BTC Reaches $1",
        description: "Bitcoin achieved dollar parity for the first time, giving it a market cap of roughly $6 million. Just months earlier, 10,000 BTC bought two pizzas.",
        short_context: Some("Bitcoin achieved dollar parity, giving it a market cap of roughly $6 million"),
        category: Milestones,
        date: "2011-02-09",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "first-halving",
        title: "First Halving",
        description: "Block reward dropped from 50 to 25 BTC. About 10.5 million BTC (50% of the total supply) had been mined in just under four years. Mined by Braiins (Slush Pool) with 457 transactions.",
        short_context: Some("Block reward dropped from 50 to 25 BTC. About 10.5 million BTC (50% of supply) had been mined in just 4 years"),
        category: Milestones,
        date: "2012-11-28",
        block: Some(210_000),
        txid: None,
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "btc-1b-market-cap",
        title: "BTC Market Cap Reaches $1 Billion",
        description: "Bitcoin crossed the billion-dollar threshold at approximately $92 per coin with 10.9 million BTC in circulation.",
        short_context: Some("Bitcoin crossed the billion-dollar threshold at ~$92 per coin with 10.9 million BTC in circulation"),
        category: Milestones,
        date: "2013-03-28",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "btc-reaches-1000",
        title: "BTC Reaches $1,000",
        description: "Driven by Chinese exchange demand on platforms like BTC China, Bitcoin crossed $1,000 for the first time. That represents a 250,000x increase from Pizza Day's implied price.",
        short_context: Some("Driven by Chinese exchange demand, Bitcoin crossed $1,000 for the first time, a 250,000x increase from Pizza Day"),
        category: Milestones,
        // Wikipedia (History of bitcoin) dates the $1,000 crossing to
        // 28 Nov 2013 at Mt. Gox, which traded at a premium and crossed
        // first. Archives said 11-27, the Almanac 11-29; both were wrong.
        date: "2013-11-28",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "second-halving",
        title: "Second Halving",
        description: "Block reward dropped from 25 to 12.5 BTC. Price was approximately $650 and would reach $20,000 within 18 months. Block contained 1,257 transactions.",
        short_context: Some("Block reward dropped from 25 to 12.5 BTC. Price was ~$650 and would reach $20K within 18 months"),
        category: Milestones,
        date: "2016-07-09",
        block: Some(420_000),
        txid: None,
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "btc-reaches-10000",
        title: "BTC Reaches $10,000",
        description: "Bitcoin breached five figures for the first time during the explosive 2017 bull run. It would double again to $20,000 in less than three weeks.",
        short_context: None,
        category: Milestones,
        date: "2017-11-28",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "btc-reaches-20000",
        title: "BTC Reaches $20,000",
        description: "The peak of the 2017 bull run. FOMO was so intense that Coinbase repeatedly crashed under traffic. A brutal 84% drawdown followed over the next year.",
        short_context: Some("The peak of the 2017 bull run. FOMO was so intense that Coinbase repeatedly crashed under traffic"),
        category: Milestones,
        date: "2017-12-17",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "third-halving",
        title: "Third Halving",
        description: "Block reward dropped from 12.5 to 6.25 BTC. Price was approximately $8,600 and would reach $69,000 within 18 months. Mined by AntPool with 3,134 transactions.",
        short_context: Some("Block reward dropped from 12.5 to 6.25 BTC. Price was ~$8,600 and would reach $69K within 18 months"),
        category: Milestones,
        date: "2020-05-11",
        block: Some(630_000),
        txid: None,
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "el-salvador-legal-tender",
        title: "El Salvador Adopts BTC as Legal Tender",
        description: "The first country to make Bitcoin legal tender. President Bukele pushed the \"Bitcoin Law\" through congress with 62 out of 84 votes. It took effect on September 7, 2021.",
        short_context: Some("The first country to make Bitcoin legal tender. President Bukele pushed the \"Bitcoin Law\" through congress"),
        category: Milestones,
        date: "2021-06-09",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "btc-ath-69k",
        title: "BTC ATH ~$69,000",
        description: "The peak of the 2021 cycle. Bitcoin's market cap briefly exceeded $1.2 trillion. The price would fall below $16,000 a year later during the FTX collapse.",
        short_context: Some("The peak of the 2021 cycle. Bitcoin's market cap briefly exceeded $1.2 trillion"),
        category: Milestones,
        date: "2021-11-10",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "first-spot-etfs",
        title: "First Spot Bitcoin ETFs Approved",
        description: "The SEC approved 11 spot Bitcoin ETFs simultaneously after a decade of rejections. Over $4 billion in volume traded on day one, marking Bitcoin's arrival in traditional finance.",
        short_context: Some("The SEC approved 11 spot Bitcoin ETFs after a decade of rejections. Over $4B in volume traded on day one"),
        category: Milestones,
        date: "2024-01-10",
        block: None,
        txid: None,
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "fourth-halving",
        title: "Fourth Halving + Runes Launch",
        description: "Block reward dropped from 6.25 to 3.125 BTC. The Runes fungible token protocol launched simultaneously, causing a massive fee spike: 37.6 BTC in fees for this single block. Mined by ViaBTC with 3,050 transactions.",
        short_context: Some("Block reward dropped from 6.25 to 3.125 BTC. The Runes protocol launched simultaneously, causing a fee spike as users minted tokens"),
        category: Milestones,
        date: "2024-04-20",
        block: Some(840_000),
        txid: None,
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "btc-reaches-100k",
        title: "BTC Breaks $100,000",
        description: "A psychological milestone 15 years in the making. From $0 to six figures, driven by institutional ETF inflows and growing mainstream adoption.",
        short_context: Some("A psychological milestone 15 years in the making. From $0 to six figures"),
        category: Milestones,
        date: "2024-12-05",
        block: None,
        txid: None,
        highlight: true,
        source: None,
    },

    // =====================================================================
    // RECORDS (all verified from DB queries)
    // =====================================================================
    HallOfFameEntry {
        slug: "largest-block",
        title: "Largest Block Ever Mined",
        // DB: height 836964, size 3,993,936 bytes, 5 txs, miner F2Pool
        description: "At 3.99 MB, block 836,964 is the largest block ever mined on Bitcoin. It contained just 5 transactions, dominated by a single massive inscription that filled nearly the entire block. Mined by F2Pool.",
        short_context: None,
        category: Records,
        date: "2024-03-30",
        block: Some(836_964),
        txid: None,
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "highest-fee-block",
        title: "Highest Fee Block (291.5 BTC)",
        // DB: height 409008, total_fees 29,153,275,103 sats, 1,962 txs, miner CKPool
        description: "Block 409,008 collected 291.5 BTC in fees, by far the highest ever. This was caused by a single transaction paying 29.1 billion sats (14.8 million sat/vB), almost certainly an accidental fat-finger fee. Mined by CKPool.",
        short_context: None,
        category: Records,
        date: "2016-04-26",
        block: Some(409_008),
        txid: Some("cc455ae816e6cdafdb58d54e35d4f46d860047458eacf1c7405dc634631c570d"),
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "most-transactions-block",
        title: "Most Transactions in a Block (12,239)",
        // DB: height 367853, tx_count 12239, size 999,956, miner Unknown
        description: "Block 367,853 packed 12,239 transactions into a pre-SegWit 1 MB block during the July-August 2015 stress test period. The transactions were mostly tiny spam outputs designed to flood the mempool.",
        short_context: None,
        category: Records,
        date: "2015-08-01",
        block: Some(367_853),
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "largest-transaction",
        title: "Largest Single Transaction (3.99 MB)",
        // DB: height 839842, largest_tx_size 3,992,821, 3 txs, miner MARA
        description: "A single transaction in block 839,842 weighed 3.99 MB, nearly filling the entire block by itself. This was a large inscription transaction that pushed the limits of what a single Bitcoin transaction can contain. Mined by MARA.",
        short_context: None,
        category: Records,
        date: "2024-04-18",
        block: Some(839_842),
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "utxo-cleanup-20k",
        title: "20,000-Input UTXO Cleanup",
        // DB: height 367885, input_count 20,894, 273 txs, miner Unknown
        description: "A single transaction swept 20,000 anyone-can-spend dust outputs left behind by the July 2015 flood attack, sending them all into one OP_RETURN. Zero BTC moved, zero fee paid. 20,000 UTXO entries removed, zero created.",
        short_context: None,
        category: Records,
        date: "2015-08-01",
        block: Some(367_885),
        txid: Some("30b3b19b4d14fae79b5d55516e93f7399e7eccd87403b8dc048ea4f49130595a"),
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "most-outputs-block",
        title: "Most Outputs in a Block (26,906)",
        // DB: height 826052, output_count 26,906, 480 txs, miner Foundry USA
        description: "Block 826,052 created 26,906 new UTXOs from just 480 transactions. Likely a mass payout or airdrop operation. Mined by Foundry USA.",
        short_context: None,
        category: Records,
        date: "2024-01-16",
        block: Some(826_052),
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "most-inscriptions-block",
        title: "Most Inscriptions in a Block (12,835)",
        // DB: height 806480, inscription_count 12,835, 766,244 bytes, miner AntPool
        description: "Block 806,480 contained 12,835 Ordinals inscriptions totaling 766 KB of embedded data during the September 2023 inscription craze. Mined by AntPool.",
        short_context: None,
        category: Records,
        date: "2023-09-06",
        block: Some(806_480),
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "most-taproot-spends",
        title: "Most Taproot Spends in a Block (22,367)",
        // DB: height 840655, taproot_spend_count 22,367, 186 txs, miner F2Pool
        description: "Block 840,655 had 22,367 Taproot (P2TR) spend inputs from just 186 transactions, with each transaction spending over 100 Taproot UTXOs on average. Mined by F2Pool four days after the fourth halving.",
        short_context: None,
        category: Records,
        date: "2024-04-24",
        block: Some(840_655),
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "fat-finger-200btc",
        title: "200 BTC Fat-Finger Fee",
        // DB: height 254642, total_fees 20,022,600,217 sats, 584 txs
        description: "Someone accidentally paid a 200 BTC fee in block 254,642, one of several infamous fat-finger incidents in Bitcoin's history. The lucky miner collected over $20,000 at the time (worth millions today).",
        short_context: None,
        category: Records,
        date: "2013-08-28",
        block: Some(254_642),
        txid: Some("4ed20e0768124bc67dc684d57941be1482ccdaa45dadb64be12afba8c8554537"),
        highlight: false,
        source: None,
    },

    // =====================================================================
    // ATTACKS & STRESS TESTS
    // =====================================================================
    HallOfFameEntry {
        slug: "overflow-bug",
        title: "184 Billion BTC Overflow Bug",
        // DB: height 74638, 3 txs. CVE-2010-5139.
        // The overflow tx was rolled back by reorg so its txid no longer exists on the canonical chain.
        description: "An integer overflow bug in block 74,638 created 184,467,440,737 BTC out of thin air, 8,784x the total supply. Satoshi published a fix (v0.3.10) within 5 hours. The good chain overtook the bad chain at block 74,691.",
        short_context: None,
        category: Attacks,
        date: "2010-08-15",
        block: Some(74_638),
        txid: None,
        highlight: true,
        source: Some(("Bitcoin Wiki: CVE-2010-5139", "https://en.bitcoin.it/wiki/CVE-2010-5139")),
    },
    HallOfFameEntry {
        slug: "mt-gox-hack",
        title: "Mt. Gox Hack",
        description: "A hacker compromised an auditor's account on Mt. Gox and dumped thousands of BTC, crashing the price from $17 to $0.01 on the exchange. About 25,000 BTC were stolen from 478 user accounts.",
        short_context: Some("A hacker compromised an auditor's account and dumped thousands of BTC, crashing the price from $17 to $0.01"),
        category: Attacks,
        date: "2011-06-19",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "mt-gox-bankruptcy",
        title: "Mt. Gox Halts Withdrawals",
        description: "The exchange that once handled 70% of all Bitcoin trading suspended all withdrawals. Weeks later, it filed for bankruptcy, revealing 850,000 BTC (~$450M at the time) had been stolen over several years.",
        short_context: Some("The exchange suspended all withdrawals, later revealing 850,000 BTC (~$450M) had been stolen. It filed for bankruptcy weeks later"),
        category: Attacks,
        date: "2014-02-07",
        block: None,
        txid: None,
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "2015-flood-attack",
        title: "July 2015 Flood Attack",
        // DB: block 364422 is from this period, 999,917 bytes
        description: "A sustained spam attack flooded the network with thousands of tiny transactions, pushing the mempool past 80,000 unconfirmed transactions and creating anyone-can-spend dust outputs that polluted the UTXO set for months.",
        short_context: None,
        category: Attacks,
        date: "2015-07-08",
        block: Some(364_422),
        txid: None,
        highlight: false,
        source: Some(("Bitcoin Wiki: July 2015 Flood Attack", "https://en.bitcoin.it/wiki/July_2015_flood_attack")),
    },
    HallOfFameEntry {
        slug: "black-thursday",
        title: "Black Thursday",
        description: "Bitcoin crashed 50% in a single day alongside global markets as COVID-19 panic hit. Liquidation cascades wiped $1 billion in leveraged positions. BTC briefly fell below $4,000.",
        short_context: Some("Bitcoin crashed 50% in hours alongside global markets as COVID panic hit. Liquidation cascades wiped $1B in leveraged positions"),
        category: Milestones,
        date: "2020-03-12",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "china-mining-ban",
        title: "China Mining Ban",
        description: "China's State Council ordered a crackdown on Bitcoin mining, triggering the largest hashrate migration in Bitcoin's history. Over 50% of global mining power relocated abroad within months.",
        short_context: Some("China ordered miners to shut down, triggering the largest hashrate migration in Bitcoin's history. Over 50% of mining moved abroad"),
        category: Milestones,
        date: "2021-05-21",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "tesla-btc",
        title: "Tesla Buys $1.5B in BTC",
        description: "Tesla's SEC filing revealed a $1.5 billion Bitcoin purchase, making it one of the first major public companies to hold BTC on its balance sheet. The announcement sent Bitcoin above $44,000 for the first time.",
        short_context: Some("Tesla's SEC filing revealed a massive Bitcoin purchase, legitimizing BTC as a corporate treasury asset"),
        category: Milestones,
        date: "2021-02-08",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "ftx-collapse",
        title: "FTX Files for Bankruptcy",
        description: "Sam Bankman-Fried's exchange collapsed after revelations of massive fraud. Approximately $8 billion in customer funds were misused. BTC dropped to $16,000, marking the cycle bottom.",
        short_context: Some("Sam Bankman-Fried's exchange collapsed after revelations of massive fraud. ~$8B in customer funds were misused. BTC dropped to $16K"),
        category: Attacks,
        date: "2022-11-11",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "inscription-fee-wave",
        title: "Inscription Fee Wave (September 2023)",
        // DB: block 806480 had 12,835 inscriptions
        description: "A massive inscription minting frenzy in September 2023 pushed block space demand to extremes. Block 806,480 alone contained 12,835 inscriptions. Fee rates spiked across the network.",
        short_context: None,
        category: Milestones,
        date: "2023-09-06",
        block: Some(806_480),
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "runes-fee-spike",
        title: "Runes Launch Fee Spike",
        // DB: block 840000, 37.6 BTC fees
        description: "The Runes protocol launched at block 840,000 (the halving block). 853 runes were etched in the first hour. Block fees hit 37.6 BTC, roughly 600x a typical block. The fee frenzy lasted several days.",
        short_context: None,
        category: Milestones,
        date: "2024-04-20",
        block: Some(840_000),
        txid: None,
        highlight: false,
        source: None,
    },

    // =====================================================================
    // PROTOCOL MOMENTS
    // =====================================================================
    HallOfFameEntry {
        slug: "p2sh-activation",
        title: "P2SH Activation (BIP-16)",
        // DB: height 173805, 106 txs
        description: "Pay-to-Script-Hash activated at block 173,805, enabling users to send to a hash of a script rather than the full script. This made multisig and complex spending conditions practical for everyday use.",
        short_context: None,
        category: Protocol,
        date: "2012-04-01",
        block: Some(173_805),
        txid: None,
        highlight: false,
        source: Some(("BIP-16", "https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki")),
    },
    HallOfFameEntry {
        slug: "strict-der-activation",
        title: "Strict DER Signature Activation (BIP-66)",
        // DB: height 363725, 1 tx (empty block), miner AntPool
        description: "BIP-66 enforced strict DER encoding for signatures at block 363,725, fixing a malleability vector. The activation caused a brief chain split when some miners hadn't upgraded.",
        short_context: None,
        category: Protocol,
        date: "2015-07-04",
        block: Some(363_725),
        txid: None,
        highlight: false,
        source: Some(("BIP-66", "https://github.com/bitcoin/bips/blob/master/bip-0066.mediawiki")),
    },
    HallOfFameEntry {
        slug: "bip66-chain-split",
        title: "BIP-66 Chain Split",
        // DB: block 363731, 1883 txs
        description: "Shortly after BIP-66 activated, miners running old software produced invalid blocks, causing a brief chain split. Miners had signaled support without actually enforcing the new rules. This led directly to BIP-9 (Version Bits), which introduced named signaling bits and defined activation windows.",
        short_context: None,
        category: Protocol,
        date: "2015-07-04",
        block: Some(363_731),
        txid: None,
        highlight: false,
        source: Some(("BIP-9", "https://github.com/bitcoin/bips/blob/master/bip-0009.mediawiki")),
    },
    HallOfFameEntry {
        slug: "cltv-activation",
        title: "CLTV Activation (BIP-65)",
        // DB: height 388381, 784 txs
        description: "CheckLockTimeVerify activated at block 388,381, enabling time-locked transactions at the script level. This was a key building block for payment channels and the Lightning Network.",
        short_context: None,
        category: Protocol,
        date: "2015-12-14",
        block: Some(388_381),
        txid: None,
        highlight: false,
        source: Some(("BIP-65", "https://github.com/bitcoin/bips/blob/master/bip-0065.mediawiki")),
    },
    HallOfFameEntry {
        slug: "csv-activation",
        title: "CSV Activation (BIP-68/112/113)",
        // DB: height 419328
        description: "CheckSequenceVerify and relative timelocks activated at block 419,328. Together with CLTV, these opcodes provided the scripting primitives needed for Lightning Network payment channels.",
        short_context: None,
        category: Protocol,
        date: "2016-07-04",
        block: Some(419_328),
        txid: None,
        highlight: false,
        source: Some(("BIP-112", "https://github.com/bitcoin/bips/blob/master/bip-0112.mediawiki")),
    },
    HallOfFameEntry {
        slug: "bitcoin-cash-fork",
        title: "Bitcoin Cash Fork",
        description: "Block 478,558 was the last block shared by Bitcoin and Bitcoin Cash. BCH forked with 8 MB blocks while Bitcoin kept 1 MB + SegWit. The \"block size war\" that defined Bitcoin's governance ended here.",
        short_context: Some("A contentious hard fork created BCH with 8MB blocks. Bitcoin kept its 1MB+SegWit approach. The \"block size war\" ended"),
        category: Protocol,
        date: "2017-08-01",
        block: Some(478_558),
        txid: None,
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "uasf-bip148",
        title: "UASF Movement (BIP-148)",
        description: "Users threatened to reject blocks that didn't signal for SegWit activation, pressuring miners to comply. The UASF movement demonstrated that economic nodes, not just miners, enforce consensus rules.",
        short_context: None,
        category: Protocol,
        date: "2017-08-01",
        block: None,
        txid: None,
        highlight: false,
        source: Some(("BIP-148", "https://github.com/bitcoin/bips/blob/master/bip-0148.mediawiki")),
    },
    HallOfFameEntry {
        slug: "segwit-activation",
        title: "SegWit Activation (BIP-141)",
        // DB: height 481824, 1,866 txs, total_fees 212,514,269 sats (~2.1 BTC)
        description: "Segregated Witness activated at block 481,824. SegWit fixed transaction malleability, increased effective block capacity, and enabled the Lightning Network. The activation block contained 6 SegWit transactions out of 1,866 total.",
        short_context: Some("<a href='https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki' target='_blank' class='text-[#f7931a] hover:underline'>BIP-141</a> activated at block 481,824. Segregated Witness fixed transaction malleability and enabled the Lightning Network"),
        category: Protocol,
        date: "2017-08-24",
        block: Some(481_824),
        txid: None,
        highlight: true,
        source: Some(("BIP-141", "https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki")),
    },
    HallOfFameEntry {
        slug: "segwit2x-cancelled",
        title: "SegWit2x Cancelled",
        description: "The New York Agreement plan to double the block size to 2 MB was abandoned due to lack of consensus. No single group (miners, businesses, or developers) was able to force the protocol change through.",
        short_context: Some("The New York Agreement plan to double the block size was abandoned due to lack of consensus. A pivotal moment for Bitcoin's governance"),
        category: Protocol,
        date: "2017-11-08",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "taproot-activation",
        title: "Taproot Activation (BIP-341)",
        // DB: height 709632, 2,043 txs, miner F2Pool
        description: "Taproot activated at block 709,632, introducing Schnorr signatures, MAST, and new script capabilities. The activation block contained 14 Taproot spends out of 2,043 transactions. Mined by F2Pool.",
        short_context: Some("<a href='https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki' target='_blank' class='text-[#f7931a] hover:underline'>BIP-341</a> activated at block 709,632. The largest Bitcoin upgrade since SegWit, enabling more private and efficient smart contracts"),
        category: Protocol,
        date: "2021-11-14",
        block: Some(709_632),
        txid: None,
        highlight: true,
        source: Some(("BIP-341", "https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki")),
    },
    HallOfFameEntry {
        slug: "first-taproot-spend",
        title: "First Taproot Key-Path Spend",
        description: "Three blocks after Taproot activated, BitGo executed the first P2TR key-path spend in block 709,635 with the message \"Thanks Satoshi!\" embedded via OP_RETURN. Multiple Bitcoin developers also made early Taproot transactions in the same block.",
        short_context: None,
        category: Protocol,
        date: "2021-11-14",
        block: Some(709_635),
        txid: Some("0bf67b1f05326afbd613e11631a2b86466ac7e255499f6286e31b9d7d889cee7"),
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "first-ordinals",
        title: "First Ordinals Inscription",
        // DB: height 767430, 2,332 txs
        description: "Casey Rodarmor inscribed a pixel art skull as inscription #0 in block 767,430, launching the Ordinals protocol. This enabled arbitrary data to be embedded directly in Bitcoin witness data. The block contained 1 inscription among 2,332 transactions.",
        short_context: None,
        category: Protocol,
        date: "2022-12-14",
        block: Some(767_430),
        txid: Some("6fb976ab49dcec017f1e201e84395983204ae1a7c2abf7ced0a85d692e442799"),
        highlight: true,
        source: None,
    },
    HallOfFameEntry {
        slug: "first-brc20",
        title: "First BRC-20 Token Deployed",
        description: "The \"ordi\" token was deployed as the first BRC-20 token by @domodata at inscription #348,020. It had a supply of 21 million with 1,000 per mint. All tokens were minted within approximately 18 hours.",
        short_context: None,
        category: Protocol,
        date: "2023-03-08",
        block: Some(779_832),
        txid: Some("b61b0172d95e266c18aea0c624db987e971a5d6d4ebc2aaed85da4642d635735"),
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "first-runes",
        title: "First Runes Etch",
        // DB: block 840000, 3,050 txs, 37.6 BTC fees
        description: "The Runes fungible token protocol went live at block 840,000, the same block as the fourth halving. 853 runes were etched within the first hour, causing extreme fee competition.",
        short_context: None,
        category: Protocol,
        date: "2024-04-20",
        block: Some(840_000),
        txid: None,
        highlight: false,
        source: None,
    },

    // =====================================================================
    // ODDITIES
    // =====================================================================
    HallOfFameEntry {
        slug: "duplicate-txids",
        title: "Duplicate Transaction IDs (BIP-30)",
        // DB: blocks 91842 and 91880 confirmed
        description: "Blocks 91,842 and 91,880 contained coinbase transactions with identical TXIDs, possible because coinbase inputs are all zeros. The duplicates overwrote the earlier UTXOs, permanently destroying 100 BTC. This led to BIP-30 (forbidding duplicates) and BIP-34 (requiring block height in coinbase).",
        short_context: None,
        category: Oddities,
        date: "2010-11-14",
        block: Some(91_842),
        txid: None,
        highlight: true,
        source: Some(("BIP-30", "https://github.com/bitcoin/bips/blob/master/bip-0030.mediawiki")),
    },
    HallOfFameEntry {
        slug: "genesis-unspendable",
        title: "Genesis Block Coinbase is Unspendable",
        description: "The 50 BTC in the genesis block can never be spent. Whether this was an intentional design choice by Satoshi or a code quirk (the genesis block wasn't inserted into the UTXO database) remains debated to this day.",
        short_context: None,
        category: Oddities,
        date: "2009-01-03",
        block: Some(0),
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "overflow-fix-block",
        title: "Overflow Bug Fix Block",
        // DB: height 74691, 2 txs
        description: "Block 74,691 is where the \"good\" chain (without the 184 billion BTC) overtook the \"bad\" chain. Miners running the patched v0.3.10 software reorganized the chain, erasing the overflow transaction within hours of its creation.",
        short_context: None,
        category: Oddities,
        date: "2010-08-15",
        block: Some(74_691),
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "first-op-return",
        title: "First Standardized OP_RETURN",
        // DB: height 291241, 578 txs, op_return_count 1, mined 2014-03-19 01:56 UTC.
        // Was height 292534 dated 2014-03-29, which is wrong twice over: that
        // block's timestamp is 2014-03-26, and it carries op_return_count 0, so
        // the description named a block containing none of what it describes.
        // Its same-day neighbours 292528/292551/292558 do have them, which is
        // consistent with a height picked by hand. 291241 is the earliest block
        // in the database with an OP_RETURN output dated on or after the 0.9.0
        // release. Hedged to "one of the first" rather than "the first": it was
        // mined 01:56 UTC on release day, so whether it precedes or follows the
        // release announcement is not something this dataset can settle, and
        // the site's own rule forbids absolutes that are not provable.
        // OP_RETURN outputs also existed long before standardisation (earliest
        // in the DB: 228,596 on 2013-03-29), a separate milestone that would
        // deserve its own entry.
        description: "Block 291,241 carries one of the first standardized OP_RETURN outputs, around the time Bitcoin Core 0.9 made 40-byte data outputs relay-standard. This gave the ecosystem a \"blessed\" way to embed small amounts of data without polluting the UTXO set.",
        short_context: None,
        category: Oddities,
        date: "2014-03-19",
        block: Some(291_241),
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "2015-utxo-dust",
        title: "Anyone-Can-Spend Dust Storm",
        description: "During the July 2015 flood attack, thousands of transactions created outputs with empty scriptPubKeys and zero value. These anyone-can-spend outputs were worthless but polluted the UTXO set, forcing every full node to track them until they were cleaned up weeks later.",
        short_context: None,
        category: Oddities,
        date: "2015-07-15",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "sighash-single-bug",
        title: "SIGHASH_SINGLE Bug",
        description: "When SIGHASH_SINGLE is used and the input index exceeds the number of outputs, Bitcoin's original code returns hash value 1 instead of failing. Any signature valid for hash(1) can spend the input. This created real anyone-can-spend outputs in the wild. Fixed for SegWit inputs but unfixable for legacy.",
        short_context: None,
        category: Oddities,
        date: "2012-07-01",
        block: None,
        txid: None,
        highlight: false,
        source: Some(("Bitcoin Wiki: OP_CHECKSIG", "https://en.bitcoin.it/wiki/OP_CHECKSIG#Procedure_for_Hashtype_SIGHASH_SINGLE")),
    },
    HallOfFameEntry {
        slug: "coldcard-seed-entropy-flaw",
        title: "COLDCARD RNG Failure",
        description: "COLDCARD firmware generated wallet seeds from a predictable fallback rather than the hardware random number generator, deriving keys from readable device state instead of real randomness. Mk2 and Mk3 were hit worst, and every current model was affected too. Seeds made this way are not merely weak, they can be found by brute force. The flaw shipped in March 2021, went unnoticed for over four years, and was being actively exploited the day it became public.",
        short_context: None,
        category: Attacks,
        date: "2026-07-30",
        block: None,
        txid: None,
        highlight: true,
        source: Some((
            "Block Engineering Report",
            "https://engineering.block.xyz/blog/predictable-rng-fallback-and-32-bit-reseed-in-coldcard-firmware",
        )),
    },
    // ── Added 2026-09-10 while reconciling the Archives against the Almanac ──
    HallOfFameEntry {
        slug: "el-salvador-law-effective",
        title: "El Salvador Bitcoin Law Takes Effect",
        description: "The Bitcoin Law passed on 9 June 2021 came into force, making Bitcoin legal tender alongside the US dollar. The government launched the Chivo wallet with a $30 sign-up incentive.",
        short_context: Some("Bitcoin became legal tender alongside the US dollar. The government launched the Chivo wallet"),
        category: Milestones,
        // Distinct from el-salvador-legal-tender, which is the law passing.
        // Passage and commencement are three months apart; both are events.
        date: "2021-09-07",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "ordinals-launch",
        title: "Ordinals Protocol Launch",
        description: "Casey Rodarmor released Ordinal Theory and the ord client, giving witness-data inscriptions a numbering scheme and a wallet.",
        short_context: Some("Casey Rodarmor launched Ordinal Theory, enabling NFT-like inscriptions in Bitcoin witness data"),
        category: Milestones,
        // first-ordinals (2022-12-14) is the first inscription, five weeks
        // earlier. The protocol's release and its first use are separate.
        date: "2023-01-21",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "btc-reaches-73k",
        title: "BTC Reaches $73,000",
        description: "A new all-time high driven by spot ETF inflows in the months after approval.",
        short_context: Some("A new all-time high driven by ETF inflows. Bitcoin surpassed silver's market cap"),
        category: Milestones,
        date: "2024-03-14",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "btc-ath-126k",
        title: "BTC ATH ~$126,000",
        description: "The current all-time high. Bitcoin's market capitalisation passed $2.5 trillion.",
        short_context: Some("The current all-time high. Bitcoin's market cap surpassed $2.5 trillion"),
        category: Milestones,
        date: "2025-10-06",
        block: None,
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "largest-difficulty-drop",
        title: "Largest Downward Difficulty Adjustment",
        // DB: retarget at block 689,472 on 2021-07-03, difficulty down 27.94%
        // on the previous period, the largest downward adjustment in chain
        // history (second is -18.03% at block 151,200, 2011-10-31). The
        // surrounding collapse, also measured: difficulty peaked at 2.50e13 in
        // mid-May 2021 and bottomed at 1.37e13 in late July, a 45% fall, and
        // the week from 2021-06-21 produced 659 blocks against 1,008 expected.
        description: "Difficulty fell 27.94% at block 689,472, the largest downward adjustment in Bitcoin's history. Hashrate left the network faster than difficulty could respond after China's mining crackdown, and blocks slowed to roughly 15 minutes apart before the retarget corrected it.",
        short_context: Some("Difficulty fell 27.94% at block 689,472, the largest downward adjustment in Bitcoin's history"),
        category: Oddities,
        date: "2021-07-03",
        block: Some(689_472),
        txid: None,
        highlight: false,
        source: None,
    },
    HallOfFameEntry {
        slug: "bip110-chain-split",
        title: "BIP-110 Chain Split",
        // DB-verified 2026-09-10: block 961,632 timestamp 2026-08-08 19:35:55
        // UTC, miner AntPool, version bit 4 NOT set. Peak support 2.53% in
        // retarget period 476 (from block 959,616, 2026-07-25), 51 of 2,016
        // blocks, every signalling block an OCEAN template. Zero of the 4,711
        // blocks since the window opened have set bit 4.
        description: "BIP-110 proposed capping transaction outputs at 34 bytes through a modified BIP9 soft fork on version bit 4. It had no failure state, only mandatory signalling: from block 961,632 every block had to set bit 4 or enforcing nodes would reject it. AntPool mined 961,632 without the bit, mainnet accepted it, and the enforcing nodes forked onto a minority chain that inherited mainnet's difficulty at a fraction of its hashrate. Support had peaked at 2.53%, supplied almost entirely by OCEAN templates.",
        short_context: Some("Mandatory bit-4 signalling opened at block 961,632. AntPool mined it without the bit and enforcing nodes forked onto a minority chain"),
        category: Attacks,
        date: "2026-08-08",
        block: Some(961_632),
        txid: None,
        highlight: false,
        source: None,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Structural invariants for the curated entries.
    ///
    /// These are hand-written constants, and the one defect found in the
    /// 2026-09-08 audit was invisible to the compiler: `first-op-return`
    /// pointed at a block containing no OP_RETURN outputs and carried a date
    /// three days off its block.
    ///
    /// `date` is the date of the **event**, not of the block, which is the
    /// convention the data already followed and is documented on the field
    /// itself in `types.rs`. So `overflow-fix-block` dating the incident to
    /// 2010-08-15 while block 74,691 was mined just after UTC midnight on the
    /// 16th is correct, not a discrepancy to reconcile.
    ///
    /// Block-height and date agreement cannot be asserted here, because it
    /// needs the database and unit tests do not have one. Re-check it by hand
    /// after editing an entry, printing each block's own date to compare
    /// against the `date` field:
    ///
    ///     sqlite3 bitcoin_stats.db "SELECT height,
    ///       date(timestamp,'unixepoch') FROM blocks WHERE height IN (...);"
    ///
    /// A one-day disagreement is expected, per the convention above. Anything
    /// larger means the height is probably wrong. The query is written out
    /// here rather than referenced because `notes/` is gitignored, so a
    /// pointer into it is dead in a fresh clone.
    #[test]
    fn entries_are_structurally_sound() {
        assert!(!HALL_OF_FAME.is_empty());

        let mut slugs = HashSet::new();
        for e in HALL_OF_FAME {
            assert!(
                slugs.insert(e.slug),
                "duplicate slug {:?}: slugs are deep-link anchors and must be unique",
                e.slug
            );
            assert!(
                !e.slug.is_empty()
                    && !e.title.is_empty()
                    && !e.description.is_empty(),
                "entry {:?} has an empty required field",
                e.slug
            );
            assert!(
                e.slug.chars().all(|c| c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || c == '-'),
                "slug {:?} is not URL-safe kebab-case",
                e.slug
            );

            // Dates are rendered and sorted as strings, so the format is load-bearing.
            let parsed = chrono::NaiveDate::parse_from_str(e.date, "%Y-%m-%d");
            assert!(
                parsed.is_ok(),
                "entry {:?} has an unparseable date {:?}",
                e.slug,
                e.date
            );
            let date = parsed.unwrap();
            assert!(
                date >= chrono::NaiveDate::from_ymd_opt(2009, 1, 3).unwrap(),
                "entry {:?} predates the genesis block",
                e.slug
            );

            if let Some(txid) = e.txid {
                assert_eq!(
                    txid.len(),
                    64,
                    "entry {:?} txid is not 64 chars",
                    e.slug
                );
                assert!(
                    txid.chars()
                        .all(|c| c.is_ascii_hexdigit()
                            && !c.is_ascii_uppercase()),
                    "entry {:?} txid is not lowercase hex",
                    e.slug
                );
            }
            if let Some((label, url)) = e.source {
                assert!(
                    !label.is_empty(),
                    "entry {:?} has an empty source label",
                    e.slug
                );
                assert!(
                    url.starts_with("https://"),
                    "entry {:?} source url is not https: {url}",
                    e.slug
                );
            }
        }
    }

    /// A height above the chain tip would render a dead block link.
    #[test]
    fn block_heights_are_plausible() {
        for e in HALL_OF_FAME {
            if let Some(h) = e.block {
                assert!(
                    h < 2_000_000,
                    "entry {:?} block height {h} is beyond any plausible tip",
                    e.slug
                );
            }
        }
    }
}
