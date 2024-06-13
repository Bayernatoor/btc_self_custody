[Multi-Signature Wallet Setup](#multi-sig-setup)

At a high level, a Multisignature wallet requires multiple **signers** to approve a transaction. You'll often see it refered to 
as a M-of-N wallet. Where M signers out of N signers are required to send a transaction (e.g. 2 of 3 or 10 of 15 etc..) 

<br> 

Multisig wallets offer additional security over a Singlesig wallet since multiple private keys are required to make a transaction.
These keys could be stored on different hardware in different locations (different countries), offering a substantial security improvement
over a typical Singlesig key. That being said, we must always tread carefully when use more complex setups as not to shoot ourselves
in the foot...as the old adage goes, Keep It Simple Stupid. 

<br> 

**Setting up a MultiSig**

Important considerations: 

<br> 

**1\.** How many co-signers (N) will be used?

<br>

**2\.** How many co-signers (M) will be required to approve a transaction? 

<br>

**3\.** What hardware will you use to store the private keys. I recommend using the latest Coldcard (Mk4 as of this writing) but to eliminate vendor risk you can chose hardware from various manufacturers (make sure they support Multisig).
        Some more options can be found on **[The Bitcoin Hole](https://thebitcoinhole.com/hardware-wallets)**.

<br>

**4\.** 1 MicroSD per Coldcard (used to create an encrypted backup and save the passphrase), plus 1 more for the Multisig setup.

<br>

**5\.** You'll need to obtain each wallets XPUB, XFP(extended fingerprint) and the key derivation path. No worries, the guide below will cover all of this. 

<br>

For this guide we will set up a 2-of-3 Multisig using 3 Mk4 Coldcards, we'll then import it into Sparrow Wallet for easier visualization and support.

<br>

**Preparing your Coldcards**

<br>

**1\.** Prepare each of the N Coldcards (signing devices) that will be used in the Multisig quorum. If you've forgotten the steps, please read **Setting up the Coldcard** in the **[Intermediate Guide](http://localhost:3000/guides/intermediate/hardware-wallet#setting-up-the-coldcard)**. 
        Remember to write down the private key, passphrase and XFP (extended fingerprint) of each wallet you create. 

<br>

**Note:** *The Coldcard does not remember your passphrase, you must enter it everytime you turn on the device. To do so enter your PIN then select ***Passphrase --> Restore Saved***. 
        The Passphrase is now in effect until you logout. For more detailed information on passphrases see the* **[Coldcard Docs](https://coldcard.com/docs/passphrase/#using-a-saved-passphrase)**

<br>

**2\.** To setup this Multisig, We'll be using Coldcards Air-Gapped Multisig tool. Insert an empty MicroSD card into the first Coldcard (order does not matter). Select ***Settings --> Multisig Wallets --> Export XPUB***. A **ccxp** file will be
        created save it to the MicroSD card. This files contains all the necessary information for setting up a Multisig. Repeat these steps for each Coldcard.
        
<br>

**3\.** There's no need to do the above step on the final Coldcard. Instead, after inserting the MicroSD card select ***Settings --> Multisig Wallets --> Create Airgapped***.
        You'll be presented with the screen below, press **1**.

<br>

<a href="#">
    <img id="coldcard_air_gapped" src="./../../../coldcard_air_gapped.png" alt="coldcard_air_gapped_screen" width="400"/> 
</a>

<br>

**4\.** You'll now select the **M** value, which is the number of co-signers required to approve a signature. The **N** value is based upon the number of **ccxp** files present on the MicroSD card. Press ***7 or 9*** to change the
        M value 

<br>

<a href="#">
    <img id="coldcard_m_of_n" src="./../../../coldcard_m_of_n.png" alt="coldcard_mofn_screen" width="400"/> 
</a>

<br>

**5\.** Press OK, you'll be presented with the new wallet information. Confirm it and 2 new files will be exported to your MicroSD card. A Coldcard multisig wallet config file (used to imported into the other Coldcards)
        and an Electrum skeleton wallet (used to import into Sparrow wallet). 

<br>

**6\.** Eject the MicroSD card, now insert it into the other Coldcard, go to  ***Settings --> Multisig Wallets --> Import from file***.

<br>

**Note:** *remember to enter your passphrase before importing the multsig config, otherwise it wont work since you'll be trying to apply it to a different private key*

<br>

**Adding your Multisig wallet to Sparrow**

<br>

**1\.** From the Sparrow toolbar click on ***File --> New Wallet***. After naming your wallet you shoul see this page: 

<br>

<a href="#">
    <img id="sparrow_multisig" src="./../../../sparrow_wallet_multisig.png" alt="sparrow_Wallet_multisig" width="600"/> 
</a>

<br>

**2\.** Set the **Policy Type** to **Multi Signature**.

<br>

**3\.** The slider to the right can be set to whatever **M-of-N** you chose. I've set it to **2-of-3** in the screenshot above.

<br>

**4\.** Recommended **Script Type** should be **Native SegWit (P2WSH)**

<br>

**5\.** Below **Keystores** you'll see tabs corresponding to the number of co-signers (N) that you selected. There are multiple 
        ways to add wallet information, if you are using hardware other then Coldcards you'll want to check their documentation on how to connect
        to Sparrow. 

<br>

**6\.** Insert the MicroSD card with the **ccxp** wallet files into your computer. Each **ccxp** file corresponds to one Coldcard. To import them into Sparrow
        Click on ***Keystore 1 --> Air-Gapped Hardware Wallet*** in the next menu locate **Coldcard Multisig**  and click on ***Import File***. **Keystore 1** will 
        populate with the wallet information. The **Label field** identifies that specific Coldcard, name it what you like. Repeat this step for each subsequent **Keystore**. 

<br>

**Importing the Coldcard ccxp file:**

<a href="#">
    <img id="sparrow_multisig_import" src="./../../../sparrow_multisig_import.png" alt="sparrow_multisig_import" width="600"/> 
</a>

<br>

**1 of 3 Keystores imported**

<a href="#">
    <img id="sparrow_multisig_keystore" src="./../../../sparrow_multisig_keystore.png" alt="sparrow_multisig_keystore" width="600"/> 
</a>

<br>

**7\.** When you're done importing all the **Keystores**, press **Apply**. You can add a password to your Sparrow Wallet if you wish. This would prevent someone with access to your computer from opening your wallet on Sparrow.

<br>

<a href="#">
    <img id="sparrow_multisig_ready_to_import" src="./../../../sparrow_multisig_ready_to_import.png" alt="sparrow_multisig_ready_to_import" width="600"/> 
</a>

<br>

**8\.** You'll now we prompted to backup your Multisig Wallet. I highly recommend that you **Save PDF**. This PDF contains the necessary information (Wallet Output Descriptor) to reconstruct your Multisig wallet in Sparrow (or other software). 
        It does not contain any private key information but should still be kept in a private and secure location.  

<br>

<a href="#">
    <img id="sparrow_multisig_backup" src="./../../../sparrow_multisig_backup.png" alt="sparrow_multisig_backup" width="600"/> 
</a>

<br>

**9\.** Once you've saved the PDF, click on "Ok" to finish the setup process, the tabs on the left should become accessible. 

<br>

Congrats, you've set up your first Multisignature wallet using Coldcards and Sparrow Wallet.
