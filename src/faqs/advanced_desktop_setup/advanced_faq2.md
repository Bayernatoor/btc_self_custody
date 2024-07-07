### Backups and Storage

<h4 class="text-2xl pb-4 text-[#f7931a] font-semibold">Backup Best Practices</h4>

Before we begin setting up our Multisig, I believe that it's important to first go over our backup and storage solutions. 
With this in mind, the setup process should make more sense. And don't worry, if you don't understand something below, It'll all be made clear during the setup, I promise! 

<br>

Setting up a Multisignature wallet is a great step in protecting your Bitcoin, 
but it's just as important to keep your Seed Words and Passphrases safe. You'll also want to keep your Wallet Output Descriptor 
backed up and in a safe location since it will be required to re-create the Multisig wallet.
To recover the funds locked in a Multisig wallet you will need **M** private keys (e.g. 2 if your setup is 2-of-3), as well as the **XPUBs of all 3 wallets**
(Wallet Output Descriptors).

<br>

<h5 class="text-2xl pb-2 text-[#f7931a] font-semibold">DO NOTs:</h5>

**1\.** Do not take digital photos of your Seed Words.

<br>

**2\.** Do not store your Seed Words on an internet connected device.

<br>

**3\.** Do not create/invent new backup methods, make sure you follow all the latest standards. DIY security is a bad idea.

<br>

<h5 class="text-2xl pb-2 text-[#f7931a] font-semibold">DOs:</h5>

**1\.** Write down your 12 or 24 Seed Words, your Passphrase and your extended Fingerprint on **separate pieces of paper**. 
        Passphrase + Fingerprint can be stored together, but stored apart from the Seed Words.

<br>

**2\.** On the Coldcard, create an **encrypted backup** of the Seed Words and save it to a MicroSD card. Also, save your Passphrase to a separate MicroSD card. 

<br>

**3\.** For each Coldcard you've setup add the 12 or 24 Seed Words to a Seedplate or other steel backup solution. You may also want to save your Passphrases in steel. 

<br>

**4\.** When finalizing the Multisig wallet setup in Sparrow save the **Wallet Output descriptor** as a PDF and keep this file safe. 
        Funds cannot be stolen with this file but it's required to re-create the Multisig wallet.
        You may also export a text file containing the XPUBS and Extended Fingerprints of the Multisig wallet.

<br>

<a href="./../../../sparrow_wallet_export.png" target="_blank">
    <img id="sparrow_wallet_export" src="./../../../sparrow_wallet_export.png" alt="sparrow_wallet_export" width="600"/> 
</a>

<br>

<p class="text-2xl pb-2 text-white font-semibold">Exported Multisig File</p>

<a href="./../../../wehodlbtc_xpub_backup.png" target="_blank">
    <img id="xpub_backup" src="./../../../wehodlbtc_xpub_backup.png" alt="xpub_backup" width="800"/> 
</a>

<br>

<h5 class="text-2xl pb-2 text-[#f7931a] font-semibold">Storage Best Practices</h5>

Backing everything up is one thing, storing it safely is another. You'll want to make sure to eliminate any single points of failure.
We must consider the following: fire, flooding and other natural disasters, 5$ wrench attacks, wear and tear and human errors. 

<br>

Don't store everything in the same place. Ideally, store each Seedplate in a different location and potentially in a separate country (if you have the means).
The Passphrase to each wallet can be stored alongside its extended Fingerprint but do not store it along with the Seed Words. 

<br>

Your digital encrypted backups are great but digital material can become corrupted or damaged. Make sure to use 
**<a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://store.coinkite.com/store/microsd-cc">industrial grade MicroSD cards<a>**, these are relatively inexpensive 
so you can create multiple backups if you wish and store them separately in different locations.

<br>

Ultimately, you must decide what your threat level is and what level of security you require.
Do not overcomplicate this for the sake of wanting **"The Best"** setup. Just KISS, **<a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://en.wikipedia.org/wiki/KISS_principle">Keep It Simple Stupid<a>**.

<br>

***More advanced solutions can be found below***


