[Advanced Solutions](#advanced-solutions)

***NOTE: These can add significant complexity to your setup so proceed with caution. YOU HAVE BEEN WARNED!!*** 

<br>

For those with unique security models or for the curious, here are some advanced suggestions to help you keep your bitcoin safe.

<br>

**Duress/Decoy Wallets:**

There are 2 accepted ways of creating "Duress Wallets" on the Coldcard. These wallets can be used to
satisfy an attacker by providing them with access to a wallet that has some funds but not 
your entire stack. 

<br>

**The Passphrase Method:**

We used a passphrase to modify the original private key loaded on the Coldcard.
Until you add the passphrase, you're dealing with an entirely different bitcoin wallet
and since the Coldcard has no idea that the private keys required for your Multisig requires a passphrase
an attacker who happens to get access to your Coldcard would also be oblivious. We can leverage this 
to prevent total fund loss. To do so deposit a small amount of bitcoin directly to an address 
associated with that wallet's private key it can than serve as a decoy and keep the remaining funds safe.

<br>

**The Duress PIN + Duress wallet:**

The Coldcard lets you set a Duress PIN, which gives you access to an entirely separate Duress Wallet.
It acts and behaves similarly to the original wallet and can be loaded with some bitcoin that you
are willing to lose. More information on this can be found in **[Coldcard's Documention](https://coldcard.com/docs/settings/#duress-pin)**.

<br>

**SeedXOR:**

Splitting your seed words into multiple parts is a VERY bad idea as explained in this **[short video](https://www.youtube.com/watch?v=p5nSibpfHYE&t=3s).**
SeedXOR enables you to store your seed in two or more parts without negatively affecting its resilience while also increasing your security. 

<br>

You can SeedXOR all three of your Coldcard's seed words and store each part in different locations. This would greatly increase your security
as all parts would need to be recombined to recreate the original seed.

<br>

To understand how this works please visit the **[official SeedXOR documentation](https://seedxor.com/)**

<br>

**Honorable mention ---> Coldcard HSM mode and CKBunker:**

This Coldcard feature is amongst one of the more advanced so I won't dive into the details but it's worth checking out
if you find yourself needing to sign transactions without physically handling the Coldcard device. You can find the
**[official docs here](https://coldcard.com/docs/hsm/)**.




