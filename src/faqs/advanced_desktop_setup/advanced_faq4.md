[Receiving & Spending](#advanced-receiving-and-spending)

<h4>Receiving to a Multisig Wallet</h4>

Receiving bitcoin to a Multisig wallet is as easy as with a Singlesig. We'll use Sparrow wallet to obtain an address. We can share that address with the sender or use it to 
send bitcoin to it ourselves.

Click on **Receive**, add a **Label** to track source of funds (only known to you), and **copy** the address.

<a href="./../../../receiving_to_multisig.png" target="_blank">
    <img id="sparrow_wallet_multisig_receive" src="./../../../receiving_to_multisig.png" alt="sparrow_wallet_multisig_receive" width="600"/> 
</a>

When your Bitcoin node detects the transaction, Sparrow will display it under the **Transactions** tab. Once it receives at least 1 confirmation, it can be 
considered received and protected by your Multisig setup. However, it's recommended to wait for 6 confirmations, before considering it to be settled and irreversible.

<a href="./../../../transaction_received.png" target="_blank">
    <img id="sparrow_wallet_multisig_received" src="./../../../transaction_received.png" alt="sparrow_wallet_multisig_received" width="600"/> 
</a>

<h4>Sending from a Multisig Wallet</h4>

Sending bitcoin from a Multisig wallet is more difficult then a Singlesig wallet. This is especially true if you've stored your Signing devices
(Coldcard, etc...) in separate locations, as you will need physical access to at least **M** devices to succesfully sign a transaction. 

**1\.** First, we'll create a **PSBT** (pre-signed bitcoin transaction) with Sparrow wallet. This **PSBT** will then be imported into **M of N Coldcards** (e.g. 2 if your setup is 2-of-3)
        to be signed. 

The steps for creating the transaction are the same as any other. First, click on **Send**. Second, in the **Pay To** field add the receivers address. Third, set a 
**Label** to identify the transaction. Fourth, set the **Amount** you want to send. Fifth, set your **Feerate** (I am in no rush so I set it low). After verifying everything,
click on **Create Transaction**.

<a href="./../../../sending_multisig_transaction.png" target="_blank">
    <img id="sparrow_wallet_multisig_send" src="./../../../sending_multisig_transaction.png" alt="sparrow_wallet_multisig_send" width="600"/> 
</a>

**2\.** On the next screen you'll have the opportunity to verify your transaction. Feel free to review the inputs and outputs on the left hand side. Confirm that the 
        receiving address is correct. Click on **Details** to view specific/technical details about the transaction. Under **Signatures** you'll find the wallet 
        responsible for signing, in this case it's our Multisig wallet. When you're ready, click on **Finalize Transaction for Signing**.

<a href="./../../../verify_the_transaction.png" target="_blank">
    <img id="sparrow_wallet_multisig_send_verify" src="./../../../verify_the_transaction.png" alt="sparrow_wallet_multisig_verfy" width="600"/> 
</a>

**3\.** It's now time to save this **PSBT** onto a MicroSD card and import it into the Coldcards for signing. Click on **Save Transaction**, you'll be prompted to save a file with
        a **.psbt** extension.

<a href="./../../../save_the_transaction.png" target="_blank">
    <img id="sparrow_wallet_multisig_send_save" src="./../../../save_the_transaction.png" alt="sparrow_wallet_multisig_save" width="600"/> 
</a>

**4\.** You'll need to sign this transaction using 2 of the 3 Coldcards. Make sure to have your Coldcard's Passphrase ready,
        either on the MicroSD card or ready to enter manually.

**5\.** Insert the MicroSD card into the Coldcard and enter your master PIN.

**6\.** Select **Passphrase**. If you have it saved to the MicroSD card then select **Restore Saved**, otherwise select **edit** and enter it manually.

**7\.** The Coldcard will load the Passphrase. you can confirm that the correct one was added by confirming that the **Extended Fingerprint** is correct.

**8\.** Select **Ready to Sign**. The Coldcard will ask which PSBT file you wish to sign, there should only be one at this stage. (When signing with the second Coldcard
        there will be 2, make sure to select the one that ends in  **-part.pbst**) 

**9\.** The Coldcard will display the **amount**, **destination address** and the associated **network fee**. Make sure to verify that the information displayed
        is correct.

**10\.** Press **Ok** to approve and sign the transaction. The PSBT will be signed and a new file ending in **-part.pbst** will be created.

**11\.** Repeat this step on the second Coldcard. Remember to select the PSBT file ending in **-part.pbst**

**12\.** Once you've finished signing, insert the MicroSD card back into your computer, go back to Sparrow Wallet and click on **Load Transaction**, select
         the PSBT file, it likely ends in **-part-2.psbt**.

**13\.** After loading the **PSBT**, 2 signatures will appear, one for each Coldcard or signing device. The blue **Broadcast Transaction** button will become clickable,
         click it to broadcast your transaction. 

<a href="./../../../signed_ready_to_broadcast.png" target="_blank">
    <img id="sparrow_wallet_multisig_signed_broadcast" src="./../../../signed_ready_to_broadcast.png" alt="sparrow_wallet_multisig_signed_broadcast" width="600"/> 
</a>

**14\.** Sparrow will broadcast your transaction to the network, once complete you'll see a page similar to the one below. 

<a href="./../../../transaction_sent.png" target="_blank">
    <img id="sparrow_wallet_multisig_transaction_sent" src="./../../../transaction_sent.png" alt="sparrow_wallet_multisig_transaction_sent" width="600"/> 
</a>

**Congrats, you've sent a bitcoin transaction using multiple co-signers.**
