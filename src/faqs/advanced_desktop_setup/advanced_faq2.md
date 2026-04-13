[Backups and Storage](#advanced-backups-and-storage)

<h4>Backup Best Practices</h4>

Before we begin setting up our Multisig, I believe that it's important to first go over our backup and storage solutions. 
With this in mind, the setup process should make more sense. And don't worry, if you don't understand something below, It'll all be made clear during the setup, I promise! 

Setting up a Multisignature wallet is a great step in protecting your Bitcoin, 
but it's just as important to keep your Seed Words and Passphrases safe. You'll also want to keep your Wallet Output Descriptor 
backed up and in a safe location since it will be required to re-create the Multisig wallet.
To recover the funds locked in a Multisig wallet you will need **M** private keys (e.g. 2 if your setup is 2-of-3), as well as the **XPUBs of all 3 wallets**
(Wallet Output Descriptors).

<h5>DO NOTs:</h5>

<ul>
    <li>Do not take digital photos of your Seed Words.</li>
    <li>Do not store your Seed Words on an internet connected device.</li>
    <li>Do not create/invent new backup methods, make sure you follow all the latest standards. DIY security is a bad idea.</li>
</ul>

<h5>DOs:</h5>

<ul>
    <li>Write down your 12 or 24 Seed Words, your Passphrase and your extended Fingerprint on <strong>separate pieces of paper</strong>. Passphrase + Fingerprint can be stored together, but stored apart from the Seed Words.</li>
    <li>On the Coldcard, create an <strong>encrypted backup</strong> of the Seed Words and save it to a MicroSD card. Also, save your Passphrase to a separate MicroSD card.</li>
    <li>For each Coldcard you've set up, add the 12 or 24 Seed Words to a Seedplate or other steel backup solution. You may also want to save your Passphrases in steel.</li>
    <li>When finalizing the Multisig wallet setup in Sparrow, save the <strong>Wallet Output descriptor</strong> as a PDF and keep this file safe. Funds cannot be stolen with this file but it's required to re-create the Multisig wallet. You may also export a text file containing the XPUBS and Extended Fingerprints of the Multisig wallet.</li>
</ul>

<a href="/guide-images/sparrow/sparrow_wallet_export.png" target="_blank">
    <img id="sparrow_wallet_export" src="/guide-images/sparrow/sparrow_wallet_export.png" alt="sparrow_wallet_export" width="600"/> 
</a>

<p>Exported Multisig File</p>

<a href="/guide-images/multisig/wehodlbtc_xpub_backup.png" target="_blank">
    <img id="xpub_backup" src="/guide-images/multisig/wehodlbtc_xpub_backup.png" alt="xpub_backup" width="800"/> 
</a>

Listen to this amazing podcast on 10Xing your Bitcoin security by <a href="https://stephanlivera.com/episode/215/" target="_blank" rel="noopener noreferrer">Stephan Livera with Michael Flaxman</a>.

<h5>Storage Best Practices</h5>

Backing everything up is one thing, storing it safely is another. You'll want to make sure to eliminate any single points of failure.
We must consider the following: fire, flooding and other natural disasters, $5 wrench attacks, wear and tear and human errors. 

Don't store everything in the same place. Ideally, store each Seedplate in a different location and potentially in a separate country (if you have the means).
The Passphrase to each wallet can be stored alongside its extended Fingerprint but do not store it along with the Seed Words. 

Your digital encrypted backups are great but digital material can become corrupted or damaged. Make sure to use 
**<a href="https://store.coinkite.com/store/microsd-cc" target="_blank" rel="noreferrer noopener">industrial grade MicroSD cards</a>**, these are relatively inexpensive 
so you can create multiple backups if you wish and store them separately in different locations.

Ultimately, you must decide what your threat level is and what level of security you require.
Do not overcomplicate this for the sake of wanting **"The Best"** setup. Just KISS, **<a href="https://en.wikipedia.org/wiki/KISS_principle" target="_blank" rel="noreferrer noopener">Keep It Simple Stupid</a>**.


