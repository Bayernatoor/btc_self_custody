### What are transaction fees?  

The bitcoin network is secured by miners and fees are paid to miners to include
your transaction in a block. Since each bitcoin block has limited space (~4mb) a fee market is created
based on market demand. As demand goes up (more transactions) the fee required to get included into a block
will increase. Fees are optional but highly recommended. In a high demand environment a transaction with no fee
will likely never get confirmed. Most wallets calculate the best fee for you based on when you'd like the transaction
to confirm, however, I always recommend double checking a <a class="underline text-blue-400 hover:text-[#3c6594]" href="https://mempool.space" target="_blank" rel="noopened noreferrer">block explorer</a>
to see the current feerate.

&nbsp; 

Fees are calculated based on the size of the transaction in kilobytes and not the value in bitcoin. This is important to keep 
in mind and has important considerations when receiving bitcoin, if you end up with many small UTXOs, the fee required to spend
them may outweigh their actual worth. Therefore, it's important to consolidate (combine) your UTXOs into larger ones when fees are low. 

&nbsp; 

For detailed information on fees, transactions and everything else bitcoin, I highly recommend the book 
<a class="underline text-blue-400 hover:text-[#3c6594]" href="https://www.amazon.com/Mastering-Bitcoin-Programming-Open-Blockchain/dp/1098150090/ref=sr_1_1?keywords=mastering+bitcoin+3rd+edition&sr=8-1" target="_blank" rel="noopener noreferrer">Mastering Bitcoin</a> by Andreas Antonopoulos & David Harding. 
