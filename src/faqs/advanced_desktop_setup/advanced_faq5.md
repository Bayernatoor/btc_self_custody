[Advanced Solutions](#advanced-solutions)

<p class="text-lg pb-4 font-semibold">Advanced Solutions</p>

***NOTE: These can add significant complexity to your setup so proceed with caution. YOU HAVE BEEN WARNED!!*** 

<br>

For those with unique security models or for the curious, here are some advanced suggestions to help you keep your bitcoin safe.

<br>

<p class="text-lg pb-4 font-semibold">Duress/Decoy Wallets</p>

There are 2 accepted ways of creating "Duress Wallets" on the Coldcard. These wallets can be used to
satisfy an attacker by providing them with access to a wallet that has some funds but not 
your entire stack. 

<br>

<p class="text-lg pb-2 font-semibold">The Passphrase Method</p>

We used a passphrase to modify the original seed words we generated on the Coldcard.
Until you add the passphrase, you're dealing with an entirely different bitcoin wallet
and since the Coldcard has no idea that the seed words required for your Multisig wallet requires a passphrase
an attacker who happens to get access to your Coldcard would also be oblivious. We can leverage this 
to prevent total fund loss. To do so deposit a small amount of bitcoin directly to an address 
associated with the Coldcard's original seed words (before adding the passphrase) 
it can than serve as a decoy and keep the remaining funds safe.

<br>

<p class="text-lg pb-2 font-semibold">The Duress PIN + Duress wallet</p>

The Coldcard lets you set a Duress PIN, which gives you access to an entirely separate Duress Wallet.
It acts and behaves similarly to the original wallet and can be loaded with some bitcoin that you
are willing to lose. More information on this can be found in **<a class="text-[#8cb4ff] underline-offset-auto" href="https://coldcard.com/docs/settings/#duress-pin">Coldcard's Documention<a>**.

<br>

<p class="text-lg pb-2 font-semibold">SeedXOR</p>

Splitting your seed words into multiple parts is a VERY bad idea as explained in this **<a class="text-[#8cb4ff] underline-offset-auto" href="https://www.youtube.com/watch?v=p5nSibpfHYE&t=3s">short video<a>**.
 SeedXOR enables you to store your seed in two or more parts without negatively affecting its resilience while also increasing your security. 

<br>

You can SeedXOR all three of your Coldcard's seed words and store each part in different locations. This would greatly increase your security
as all parts would need to be recombined to recreate the original seed.

<br>

To understand how this works please visit the **<a class="text-[#8cb4ff] underline-offset-auto" href="https://seedxor.com/">official SeedXOR documentation<a>**

<br>

<p class="text-lg pb-2 font-semibold">Honorable mention ---> Coldcard HSM mode and CKBunker</p>

This Coldcard feature is amongst one of the more advanced so I won't dive into the details but it's worth checking out
if you find yourself needing to sign transactions without physically handling the Coldcard device. You can find the
**<a class="text-[#8cb4ff] underline-offset-auto" href="https://coldcard.com/docs/hsm/">official docs here<a>**.





