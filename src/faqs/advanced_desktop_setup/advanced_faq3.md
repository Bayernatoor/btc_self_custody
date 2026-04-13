[Multisignature Wallet Setup](#advanced-multisignature-wallet-setup)

<h4>Setting up a Multisig</h4>

At a high level, a Multisignature wallet requires multiple **signers** to approve a transaction. You'll often see it refered to 
as a **M-of-N wallet**. Where **M signers** out of **N signers** are required to send a transaction (e.g. 2 of 3 or 10 of 15 etc..). 

Multisig wallets offer additional security over a Singlesig wallet since multiple private keys are required to make a transaction.
These keys can be stored on different hardware in different locations (different countries), only accessible to specific people, offering a substantial security improvement
over a typical Singlesig wallet. That being said, we must always tread carefully when using more complex setups so as not to shoot ourselves
in the foot...as the old adage goes, Keep It Simple Stupid. 

<h4>Important Considerations</h4>

<ul>
    <li>How many co-signers (N) will be used?</li>
    <li>How many co-signers (M) will be required to approve a transaction?</li>
    <li>What hardware will you use to store the private keys? I recommend using the latest Coldcard (Mk4 as of this writing) but to eliminate vendor risk you can choose hardware from various manufacturers (make sure they support Multisig). Some more options can be found on <a href="https://thebitcoinhole.com/hardware-wallets" target="_blank" rel="noopener noreferrer">The Bitcoin Hole</a>.</li>
    <li>You'll need 2 MicroSD cards per Coldcard (1 for the encrypted wallet backup and the other for the Passphrase), plus 1 more for the Multisig setup (you can technically use any MicroSD card for this, such as the one used for a Passphrase).</li>
    <li>You'll need to obtain each wallet's XPUB, XFP (extended Fingerprint) and the key derivation path. The guide below will cover all of this.</li>
</ul>

***Note: For this guide we will set up a 2-of-3 Multisig using 3 Mk4 Coldcards, we'll then import it into Sparrow Wallet for easier visualization and support.***

<h4>Preparing your Coldcards</h4>

**1\.** Prepare each of the N Coldcards (signing devices) that will be used in the Multisig quorum. If you've forgotten the steps, please read **Setting up the Coldcard** in the 
        **<a href="/guide-images/intermediate/hardware-wallet#setting-up-the-coldcard" target="_blank">Intermediate Guide</a>**. 
        Remember to write down the Seed Words, Passphrase and XFP (Extended/Master Fingerprint) of each wallet you create. 

**Note:** *The Coldcard does not remember your Passphrase, you must enter it everytime you turn on the device. To do so enter your PIN, insert the MicroSD card that stores your encrypted Passphrase
        then select **Passphrase --> Restore Saved**. The Passphrase is now in effect until you logout. 
        For more detailed information on Passphrases see the* **<a href="https://coldcard.com/docs/passphrase/#using-a-saved-passphrase" target="_blank" rel="noopener noreferrer">Coldcard Docs</a>.**

**2\.** To setup this Multisig, We'll be using Coldcards Air-Gapped Multisig tool. 
        Insert an empty MicroSD card into the first Coldcard (order does not matter). Select ***Settings --> Multisig Wallets --> Export XPUB***. A **ccxp** file will be
        created, save it to the MicroSD card. This file contains all the necessary information for setting up a Multisig. Repeat these steps for each Coldcard.

**3\.** There's no need to do the above step on the final Coldcard. Instead, after inserting the MicroSD card select ***Settings --> Multisig Wallets --> Create Airgapped***.
        You'll be presented with the screen below, press **Ok**.

<a href="/guide-images/coldcard/coldcard_air_gapped.png" target="_blank">
    <img id="coldcard_air_gapped" src="/guide-images/coldcard/coldcard_air_gapped.png" alt="coldcard_air_gapped_screen" width="400"/> 
</a>

**4\.** You'll now select the **M** value, which is the number of co-signers required to approve a signature. 
        The **N** value is the number of **ccxp** files present on the MicroSD card (Total # of Signing Devices being used). 
        Press ***7 or 9*** to change the M value. 

<a href="/guide-images/coldcard/coldcard_m_of_n.png" target="_blank">
    <img id="coldcard_m_of_n" src="/guide-images/coldcard/coldcard_m_of_n.png" alt="coldcard_m_of_n_screen" width="400"/> 
</a>

**5\.** Press OK, you'll be presented with the new wallet information. Confirm it and 2 new files will be exported to your MicroSD card. 
        A Coldcard multisig wallet config file (to be imported into the other Coldcards)
        and an Electrum skeleton wallet (used to import into Sparrow wallet). 

**6\.** Eject the MicroSD card, now insert it into the other Coldcards, go to  ***Settings --> Multisig Wallets --> Import from file.***

**Note:** *remember to enter your Passphrase before importing the multsig config, otherwise it won't work since you'll be trying to apply construct the Multisig with different private key.*

<h4>Adding your Multisig wallet to Sparrow</h4>

**1\.** From the Sparrow toolbar click on ***File --> New Wallet***. Add a name for your wallet and press **Create Wallet**.

<a href="/guide-images/multisig/sparrow_wallet_multisig_new_wallet.png" target="_blank">
    <img id="sparrow_wallet_multisig_name" src="/guide-images/multisig/sparrow_wallet_multisig_new_wallet.png" alt="sparrow_wallet_multisig_name" width="600"/> 
</a>

**2\.** Set the **Policy Type** to **Multi Signature**.

**3\.** The slider to the right can be set to whatever **M-of-N** you chose. I've set it to **2-of-3** in the screenshot above.

**4\.** Recommended **Script Type** should be **Native SegWit (P2WSH)**

**5\.** Below **Keystores** you'll see tabs corresponding to the number of co-signers (N) that you selected. There are multiple 
        ways to add wallet information, if you are using hardware other then Coldcards you'll want to check their documentation on how to connect
        to Sparrow. 

<a href="/guide-images/multisig/sparrow_new_wallet_multisig.png" target="_blank">
    <img id="sparrow_multisig" src="/guide-images/multisig/sparrow_new_wallet_multisig.png" alt="sparrow_Wallet_multisig" width="600"/> 
</a>

**6\.** Insert the MicroSD card with the **ccxp** wallet files into your computer. Each **ccxp** file corresponds to one Coldcard. To import them into Sparrow
        Click on ***Keystore 1 --> Air-Gapped Hardware Wallet*** in the next menu locate **Coldcard Multisig**  and click on ***Import File***. **Keystore 1** will 
        populate with the wallet information. The **Label field** identifies that specific Coldcard, name it what you like. Repeat this step for each subsequent **Keystore**. 

<p>Importing the Coldcard ccxp file</p>

<a href="/guide-images/multisig/sparrow_multisig_import.png" target="_blank">
    <img id="sparrow_multisig_import" src="/guide-images/multisig/sparrow_multisig_import.png" alt="sparrow_multisig_import" width="600"/> 
</a>

<p>1 of 3 Keystores imported</p>

<a href="/guide-images/multisig/sparrow_multisig_keystore.png" target="_blank">
    <img id="sparrow_multisig_keystore" src="/guide-images/multisig/sparrow_multisig_keystore.png" alt="sparrow_multisig_keystore" width="600"/> 
</a>

**7\.** When you're done importing all the **Keystores**, press **Apply**. 
        You can add a password to your Sparrow Wallet if you wish. 
        This would prevent someone with access to your computer from opening your wallet on Sparrow.

<a href="/guide-images/multisig/sparrow_multisig_ready_to_import.png" target="_blank">  
    <img id="sparrow_multisig_ready_to_import" src="/guide-images/multisig/sparrow_multisig_ready_to_import.png" alt="sparrow_multisig_ready_to_import" width="600"/> 
</a>

**8\.** You'll be prompted to backup your Multisig Wallet. I highly recommend that you **Save PDF**. 
        This PDF contains the necessary information (Wallet Output Descriptor) to reconstruct your Multisig wallet in Sparrow (or other software). 
        It does not contain any private key information but should still be kept in a private and secure location.  

<a href="/guide-images/multisig/sparrow_multisig_backup.png" target="_blank">
    <img id="sparrow_multisig_backup" src="/guide-images/multisig/sparrow_multisig_backup.png" alt="sparrow_multisig_backup" width="600"/> 
</a>

**9\.** Once you've saved the PDF, click on "Ok" to finish the setup process, the tabs on the left should become accessible. 

Congrats, you've set up your first Multisignature wallet using Coldcards and Sparrow Wallet.
