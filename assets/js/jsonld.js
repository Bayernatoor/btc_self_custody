// Schema.org JSON-LD for search engines and LLMs
(function(){var s=document.createElement('script');s.type='application/ld+json';s.textContent=JSON.stringify({
"@context":"https://schema.org",
"@graph":[
{"@type":"WebSite","name":"WE HODL BTC","url":"https://www.wehodlbtc.com","description":"Free Bitcoin self-custody guides and live blockchain analytics. Real-time network stats, fee charts, mining data, and BIP signaling tracker.","publisher":{"@type":"Organization","name":"WE HODL BTC","url":"https://www.wehodlbtc.com"}},
{"@type":"WebApplication","name":"The Bitcoin Observatory","url":"https://www.wehodlbtc.com/observatory","applicationCategory":"FinanceApplication","operatingSystem":"Web","description":"Live Bitcoin blockchain analytics dashboard with 45+ charts covering real-time network stats, fee analysis, mining pool distribution, embedded data tracking (OP_RETURN, Ordinals, Runes), and BIP signaling. Data sourced directly from a full Bitcoin Core node.","featureList":["Live blockchain dashboard with block height, difficulty, hashrate, mempool, and price","Network charts: block size, weight utilization, transaction count, TPS, block intervals, chain size, block fullness distribution, block time distribution, rapid consecutive blocks, weekday activity","Adoption metrics: SegWit percentage, Taproot outputs, witness versions, address type evolution, P2PKH sunset tracker, adoption velocity for all types, cumulative adoption milestones","Transaction metrics: RBF adoption, UTXO flow, UTXO growth rate, transaction density, batching trends, transaction type evolution","Fee analysis: total fees, avg fee per tx, median fee rate, fee rate bands (p10-p90), subsidy vs fees, fee revenue share, BTC volume, value flow, fee pressure scatter, fee spike detector, halving era comparison, protocol fee breakdown","Mining analytics: difficulty history, difficulty ribbon (miner capitulation), mining pool dominance with OCEAN template miners, mining diversity index (HHI), empty blocks by pool","Embedded data: OP_RETURN protocols (Runes, Omni Layer, Counterparty), Ordinals inscriptions, BRC-20 tokens","BIP signaling tracker: BIP-110 version bit 4, BIP-54 coinbase locktime compliance per 2,016-block period","Block Heartbeat: real-time EKG visualization of mempool and block activity","Stats Overview: record-breaking blocks with 15 extreme categories","Hall of Fame: 57+ curated notable Bitcoin blocks and transactions","Chart overlays: halvings, BIP activations, Bitcoin Core releases, historical events, USD price","Time ranges from 1 day to full chain history with per-block and daily aggregation"],"offers":{"@type":"Offer","price":"0","priceCurrency":"USD"},"provider":{"@type":"Organization","name":"WE HODL BTC","url":"https://www.wehodlbtc.com"}},
{"@type":"Dataset","name":"Bitcoin Blockchain Statistics","description":"Real-time and historical Bitcoin blockchain data including block height, mining difficulty, network hashrate, mempool size, transaction counts, SegWit/Taproot adoption rates, fee levels, and supply metrics. Covers the full chain from genesis block (2009) to present. Updated every 60 seconds from a full Bitcoin Core node with txindex.","url":"https://www.wehodlbtc.com/observatory","license":"https://creativecommons.org/licenses/by/4.0/","creator":{"@type":"Organization","name":"WE HODL BTC","url":"https://www.wehodlbtc.com"},"temporalCoverage":"2009/..","variableMeasured":[
{"@type":"PropertyValue","name":"Block Height","description":"Current Bitcoin blockchain height"},
{"@type":"PropertyValue","name":"Mining Difficulty","description":"Mining difficulty in trillions (T), adjusted every 2,016 blocks"},
{"@type":"PropertyValue","name":"Network Hashrate","description":"Estimated network hash rate in EH/s"},
{"@type":"PropertyValue","name":"SegWit Adoption","description":"Percentage of transactions using Segregated Witness"},
{"@type":"PropertyValue","name":"Taproot Adoption","description":"Number of P2TR (Taproot) outputs created per block"},
{"@type":"PropertyValue","name":"Transaction Fees","description":"Total miner fee revenue per block in BTC and satoshis"},
{"@type":"PropertyValue","name":"Mempool Size","description":"Number of unconfirmed transactions and total size in bytes"},
{"@type":"PropertyValue","name":"Bitcoin Supply","description":"Total BTC issued and percentage of 21 million cap mined"},
{"@type":"PropertyValue","name":"Block Weight Utilization","description":"Percentage of the 4 MWU block weight limit used per block"},
{"@type":"PropertyValue","name":"RBF Adoption","description":"Percentage of transactions signaling Replace-By-Fee (BIP 125)"}
]},
{"@type":"Dataset","name":"Bitcoin Embedded Data Analytics","description":"Tracking non-financial data embedded in Bitcoin transactions: OP_RETURN protocol usage (Runes, Omni Layer, Counterparty), Ordinals inscriptions, BRC-20 tokens, and Stamps. Includes per-block counts, byte volumes, block share percentages, and protocol dominance over time.","url":"https://www.wehodlbtc.com/observatory/charts/embedded","creator":{"@type":"Organization","name":"WE HODL BTC"}},
{"@type":"Dataset","name":"Bitcoin BIP Signaling Data","description":"Per-block miner signaling data for active Bitcoin Improvement Proposals. Tracks BIP-110 (relaxed OP_RETURN data limits, version bit 4, 55% activation threshold) and BIP-54 (great consensus cleanup, coinbase locktime == height-1, 95% threshold) across 2,016-block retarget periods.","url":"https://www.wehodlbtc.com/observatory/signaling","creator":{"@type":"Organization","name":"WE HODL BTC"}},
{"@type":"Article","name":"Bitcoin Embedding Protocols: A Technical Guide","headline":"Bitcoin Embedding Protocols: Runes, Ordinals, BRC-20, Stamps, Omni, Counterparty","description":"Technical comparison of data embedding protocols on Bitcoin. Covers OP_RETURN-based protocols (Runes, Omni, Counterparty), witness-based protocols (Ordinals, BRC-20), and bare multisig encoding (Stamps). Includes chronological timeline, pruning characteristics, and trade-offs.","url":"https://www.wehodlbtc.com/observatory/learn/protocols","author":{"@type":"Organization","name":"WE HODL BTC"},"about":[{"@type":"Thing","name":"Runes Protocol"},{"@type":"Thing","name":"Ordinals Inscriptions"},{"@type":"Thing","name":"BRC-20 Tokens"},{"@type":"Thing","name":"Bitcoin Stamps"},{"@type":"Thing","name":"Omni Layer"},{"@type":"Thing","name":"Counterparty"}]},
{"@type":"Article","name":"Data Methodology","headline":"WE HODL BTC Data Methodology: How We Source, Compute, and Classify Bitcoin Metrics","description":"Complete documentation of the observatory's data methodology. Covers block metrics, fee calculations, address type classification, mining pool identification, embedded protocol detection, price data sourcing, daily aggregation, and known exclusions.","url":"https://www.wehodlbtc.com/observatory/learn/methodology","author":{"@type":"Organization","name":"WE HODL BTC"},"about":[{"@type":"Thing","name":"Bitcoin Analytics"},{"@type":"Thing","name":"Blockchain Data"},{"@type":"Thing","name":"Data Methodology"}]},
{"@type":"BreadcrumbList","itemListElement":[
{"@type":"ListItem","position":1,"name":"Home","item":"https://www.wehodlbtc.com/"},
{"@type":"ListItem","position":2,"name":"Observatory","item":"https://www.wehodlbtc.com/observatory"},
{"@type":"ListItem","position":3,"name":"Network Charts","item":"https://www.wehodlbtc.com/observatory/charts/network"},
{"@type":"ListItem","position":4,"name":"Fee Charts","item":"https://www.wehodlbtc.com/observatory/charts/fees"},
{"@type":"ListItem","position":5,"name":"Mining Charts","item":"https://www.wehodlbtc.com/observatory/charts/mining"},
{"@type":"ListItem","position":6,"name":"Embedded Data","item":"https://www.wehodlbtc.com/observatory/charts/embedded"},
{"@type":"ListItem","position":7,"name":"BIP Signaling","item":"https://www.wehodlbtc.com/observatory/signaling"},
{"@type":"ListItem","position":8,"name":"Learn","item":"https://www.wehodlbtc.com/observatory/learn"},
{"@type":"ListItem","position":9,"name":"Protocol Guide","item":"https://www.wehodlbtc.com/observatory/learn/protocols"},
{"@type":"ListItem","position":10,"name":"Data Methodology","item":"https://www.wehodlbtc.com/observatory/learn/methodology"},
{"@type":"ListItem","position":11,"name":"Heartbeat","item":"https://www.wehodlbtc.com/observatory/heartbeat"},
{"@type":"ListItem","position":12,"name":"The Logbook","item":"https://www.wehodlbtc.com/observatory/stats"},
{"@type":"ListItem","position":13,"name":"Almanac","item":"https://www.wehodlbtc.com/observatory/on-this-day"},
{"@type":"ListItem","position":14,"name":"The Archives","item":"https://www.wehodlbtc.com/observatory/hall-of-fame"},
{"@type":"ListItem","position":15,"name":"The Lookout","item":"https://www.wehodlbtc.com/observatory/whale-watch"}
]},
{"@type":"BreadcrumbList","itemListElement":[
{"@type":"ListItem","position":1,"name":"Home","item":"https://www.wehodlbtc.com/"},
{"@type":"ListItem","position":2,"name":"Self-Custody Guides","item":"https://www.wehodlbtc.com/guides"}
]},
{"@type":"BreadcrumbList","itemListElement":[
{"@type":"ListItem","position":1,"name":"Home","item":"https://www.wehodlbtc.com/"},
{"@type":"ListItem","position":2,"name":"FAQ","item":"https://www.wehodlbtc.com/faq"}
]},
{"@type":"BreadcrumbList","itemListElement":[
{"@type":"ListItem","position":1,"name":"Home","item":"https://www.wehodlbtc.com/"},
{"@type":"ListItem","position":2,"name":"About","item":"https://www.wehodlbtc.com/about"}
]},
{"@type":"ItemList","name":"The Bitcoin Archives","description":"A curated collection of Bitcoin's most remarkable blocks and transactions. Covers milestones (genesis block, halvings, price ATHs), on-chain records (largest block, highest fees), protocol moments (SegWit, Taproot, Ordinals activations), attacks and stress tests, and oddities.","url":"https://www.wehodlbtc.com/observatory/hall-of-fame","numberOfItems":57,"itemListElement":[
{"@type":"ListItem","position":1,"name":"Genesis Block","url":"https://www.wehodlbtc.com/observatory/hall-of-fame#genesis-block"},
{"@type":"ListItem","position":2,"name":"First Bitcoin Transaction","url":"https://www.wehodlbtc.com/observatory/hall-of-fame#first-transaction"},
{"@type":"ListItem","position":3,"name":"Bitcoin Pizza Day","url":"https://www.wehodlbtc.com/observatory/hall-of-fame#pizza-day"},
{"@type":"ListItem","position":4,"name":"184 Billion BTC Overflow Bug","url":"https://www.wehodlbtc.com/observatory/hall-of-fame#overflow-bug"},
{"@type":"ListItem","position":5,"name":"SegWit Activation","url":"https://www.wehodlbtc.com/observatory/hall-of-fame#segwit-activation"},
{"@type":"ListItem","position":6,"name":"Taproot Activation","url":"https://www.wehodlbtc.com/observatory/hall-of-fame#taproot-activation"},
{"@type":"ListItem","position":7,"name":"First Ordinals Inscription","url":"https://www.wehodlbtc.com/observatory/hall-of-fame#first-ordinals"},
{"@type":"ListItem","position":8,"name":"Largest Block Ever Mined","url":"https://www.wehodlbtc.com/observatory/hall-of-fame#largest-block"},
{"@type":"ListItem","position":9,"name":"Highest Fee Block","url":"https://www.wehodlbtc.com/observatory/hall-of-fame#highest-fee-block"},
{"@type":"ListItem","position":10,"name":"Duplicate Transaction IDs","url":"https://www.wehodlbtc.com/observatory/hall-of-fame#duplicate-txids"}
]},
{"@type":"FAQPage","url":"https://www.wehodlbtc.com/faq","mainEntity":[
{"@type":"Question","name":"What is Bitcoin?","acceptedAnswer":{"@type":"Answer","text":"Bitcoin is money for the digital age - controlled by no one, accessible to everyone. It's a decentralized store of value and payment network secured by cryptography and proof-of-work mining."}},
{"@type":"Question","name":"Why Bitcoin?","acceptedAnswer":{"@type":"Answer","text":"Bitcoin is an exit from government-issued fiat currencies that undergo constant inflation. It is the best savings mechanism ever discovered, preserving wealth as everything priced in bitcoin trends towards zero."}},
{"@type":"Question","name":"Why Bitcoin self-custody?","acceptedAnswer":{"@type":"Answer","text":"Controlling a Bitcoin private key grants absolute control over the associated bitcoin. Self-custody restores independence and self-sovereignty, eliminating reliance on third parties like exchanges that can freeze, lose, or restrict access to your funds."}},
{"@type":"Question","name":"What is a Bitcoin transaction?","acceptedAnswer":{"@type":"Answer","text":"A Bitcoin transaction represents the transfer of value between participants on the Bitcoin network. It consists of one or more inputs (funds being spent) and one or more outputs (destinations receiving funds)."}},
{"@type":"Question","name":"What is a Bitcoin mempool?","acceptedAnswer":{"@type":"Answer","text":"A Bitcoin mempool (memory pool) is a temporary repository for unconfirmed transactions. Every node on the network maintains its own mempool, storing transactions until they are included in a mined block."}},
{"@type":"Question","name":"How does Bitcoin mining work?","acceptedAnswer":{"@type":"Answer","text":"Bitcoin mining uses specialized computers (ASICs) to solve proof-of-work puzzles approximately every 10 minutes. When a miner finds a valid solution, they add a new block to the blockchain and earn the block subsidy plus transaction fees."}},
{"@type":"Question","name":"What are Bitcoin transaction fees?","acceptedAnswer":{"@type":"Answer","text":"Transaction fees compensate miners for including transactions in blocks. Each block has limited space (~4 MB weight), creating a fee market. As demand increases, fees rise. Fees are measured in satoshis per virtual byte (sat/vB)."}},
{"@type":"Question","name":"What are Bitcoin private and public keys?","acceptedAnswer":{"@type":"Answer","text":"A private key is a secret value (often represented as 12 or 24 words) that grants control over bitcoin. A public key is derived from the private key and used to generate Bitcoin addresses for receiving funds. Never share your private key."}},
{"@type":"Question","name":"What is a Bitcoin wallet?","acceptedAnswer":{"@type":"Answer","text":"A Bitcoin wallet is software that stores your private keys and enables you to send and receive bitcoin. It does not store actual bitcoins - it stores the cryptographic keys that authorize transactions on the network."}}
]},
{"@type":"ItemList","name":"Bitcoin Self-Custody Guides","itemListElement":[{"@type":"HowTo","position":1,"name":"Basic Bitcoin Self-Custody Guide","description":"Set up a mobile or desktop Bitcoin wallet for self-custody. Covers Blue Wallet, Green Wallet, and Sparrow Wallet with step-by-step instructions."},{"@type":"HowTo","position":2,"name":"Intermediate Bitcoin Self-Custody Guide","description":"Set up a Coldcard hardware wallet and connect it to your own Bitcoin node using Start9, MyNode, or RaspiBlitz for enhanced security and privacy."},{"@type":"HowTo","position":3,"name":"Advanced Bitcoin Self-Custody Guide","description":"Create a 2-of-3 multisig wallet with multiple signing devices, steel seed backups, and geographic separation for maximum Bitcoin security."}]}
]
});document.head.appendChild(s)})();
