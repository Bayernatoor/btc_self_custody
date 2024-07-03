### What is a bitcoin transaction?

Simply put, a bitcoin transaction represents the transfer of value between 1 or more participants of the bitcoin network.

<br>

A bitcoin transaction consists of 1 or more inputs and 1 or more outputs. Ultimately, all inputs/ouputs are
called ***unspent transaction outputs*** or UTXO for short. The collection of all UTXOs is called the UXTO set, which 
can be calculated & verified by anyone running a bitcoin node. This UTXO set is equal to all "bitcoins" or more accurately "satoshis" in circulation.  

<br>

The UTXO set is constantly growing and shrinking depending on the transactions being broadcasted to the network. 
UTXOs themselves are indivisible so in order to divide them, new UTXOs must be created. 
You can consolidate multiple UTXOs into a single one or you can divide 1 UTXO into multiple. All of this 
is done via bitcoin transactions.

<br>

**For instance**, imagine your friend wants to send you 0.00158955 BTC or 158,955 satoshis. However, he only have one UTXO of 0.002 BTC (200,000 satoshis) in his wallet.
The transaction he creates will include one input using his UTXO of 200,000 satoshis. The output will likely consist of three UTXOs: 
158,955 satoshis sent to your address, a miner fee, and any remaining amount (change) sent to a new address he controls.
In this simple example, one UTXO of 200,000 satoshis was spent, resulting in the creation of three new UTXOs.
