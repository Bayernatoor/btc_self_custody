### Advanced Solutions

<h4 class="text-2xl pb-4 text-[#f7931a] font-semibold">Advanced Solutions</h4>

***NOTE: These can add significant complexity to your setup so proceed with caution. YOU HAVE BEEN WARNED!!*** 

<br>

For those with unique security models or for the curious, here are some advanced suggestions to help keep your bitcoin safe.

<br>

<h4 class="text-2xl pb-4 text-[#f7931a] font-semibold">Duress/Decoy Wallets</h4>

There are two widely recognized methods for setting up Duress Wallets on the Coldcard. 
These wallets allow you to present an attacker with access to a wallet containing some funds, while keeping your main holdings secure.

<br>

<h5 class="text-xl pb-2 text-white font-semibold">The Passphrase Method</h5>

We used a Passphrase to modify the original Seed Words generated on the Coldcard. 
Until you add the Passphrase, you're dealing with an entirely different Bitcoin wallet. 
Since the Coldcard doesn’t recognize that the Seed Words for your Multisig wallet require a Passphrase, 
an attacker who gains access to your Coldcard would also be unaware. We can leverage this to prevent total fund loss. 
To do so, deposit a small amount of Bitcoin directly to an address associated with the Coldcard's original Seed Words (before adding the Passphrase). 
It can then serve as a decoy and keep the remaining funds safe.

<br>

<h5 class="text-xl pb-2 text-white font-semibold">The Duress PIN + Duress Wallet</h5>

The Coldcard allows you to set a Duress PIN, which grants access to a separate Duress Wallet. 
It functions similarly to the original wallet and can be loaded with some Bitcoin that you are willing to lose. 
More information on this can be found in 
<a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://coldcard.com/docs/settings/#duress-pin" target="_blank" rel="noopener noreferrer">Coldcard's Documentation<a>.

<br>

<h5 class="text-xl pb-2 text-white font-semibold">SeedXOR</h5>

Splitting your Seed Words into multiple parts is a VERY bad idea, as explained in this 
<a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://www.youtube.com/watch?v=p5nSibpfHYE&t=3s" target="_blank" rel="noopener noreferrer">short video<a>. 
SeedXOR allows you to store your seed in two or more parts without compromising its resilience while also improving your security.

<br>

You can SeedXOR all three of your Coldcard's Seed Words and store each part in different locations. 
This greatly increases your security as all parts would need to be recombined to recreate the original seed.

<br>

To understand how this works, please visit the <a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://seedxor.com/" target="_blank" rel="noopener noreferrer">official SeedXOR documentation<a>.

<br>

<h5 class="text-xl pb-2 text-white font-semibold">Honorable Mention — Coldcard HSM Mode and CKBunker</h5>


This Coldcard feature is among the more advanced, so I won’t dive into the details, 
but it's worth checking out if you need to sign transactions without physically handling the Coldcard device. 
You can find the <a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://coldcard.com/docs/hsm/" target="_blank" rel="noopener noreferrer">official docs here<a>.



