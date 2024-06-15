[Receiving & Spending](#receiving-and-spending)

<p class="text-lg pb-4 font-semibold">Receiving to a Multisig Wallet</p>

Receiving bitcoin to a Multisig wallet is as easy as with a Singlesig. We'll use Sparrow wallet to obtain an address 
and simply send funds to it. 

<br>

Click on **Receive**, add a **Label** to track source of funds (only known to you), and **copy** the address.

<br>

<a href="./../../../receiving_to_multisig.png" target="_blank">
    <img id="sparrow_wallet_multisig_receive" src="./../../../receiving_to_multisig.png" alt="sparrow_wallet_multisig_receive" width="600"/> 
</a>
 
<br>

Once the transaction appears in a mempool, Sparrow will display it under the **Transactions** tab. Once it receives at least 1 confirmation, it can be 
considered received and protected by your Multisig setup.<br>

<br>

<a href="./../../../transaction_received.png" target="_blank">
    <img id="sparrow_wallet_multisig_received" src="./../../../transaction_received.png" alt="sparrow_wallet_multisig_received" width="600"/> 
</a>
 
<br>

<p class="text-lg pb-2 font-semibold">Sending from a Multisig Wallet</p>

Sending bitcoin from a Multisig wallet is more difficult then a Singlesig wallet. This is especially true if you've stored your Signing devices
(Coldcard, etc...) in separate locations, as you will need physical access to at least **M** devices to succesfully sign a transaction. 

<br> 

**1\.** First, we'll create a PSBT (pre-signed bitcoin transaction) with Sparrow wallet. This PSBT will then be imported into **M of N Coldcards** (e.g. 2 if your setup is 2-of-3)
        to be signed. 

<br>

The steps for creating the transaction are the same as any other. First, click on **Send**. Second, in the **Pay To** field add the receivers address. Third, set a 
**Label** to identify the transaction. Fourth, set the **Amount** you want to send. Fifth, set your **Feerate** (I am in no rush so I set it low). After verifying everything,
click on **Create Transaction**.

<br>

<a href="./../../../sending_multisig_transaction.png" target="_blank">
    <img id="sparrow_wallet_multisig_send" src="./../../../sending_multisig_transaction.png" alt="sparrow_wallet_multisig_send" width="600"/> 
</a>
 
<br>

**2\.** On the next screen you'll have the opportunity to verify your transaction. Feel free to view the inputs and ouputs on the left hand side. Confirm that the 
        receicing address is correct. Click on **Details** to view specific/technical details about the transaction. Under **Signatures** you'll find the wallet 
        responsible for signing, in this case it's our Multsig wallet. Finally, click on **Finalize Transaction for Signing**.

<br>

<a href="./../../../verify_the_transaction.png" target="_blank">
    <img id="sparrow_wallet_multisig_send_verify" src="./../../../verify_the_transaction.png" alt="sparrow_wallet_multisig_verfy" width="600"/> 
</a>
 
<br>

**3\.** It's now time to save this PSBT and import it into the Coldcards for signing. Click on **Save transaction**, you'll be prompted to save a file with
        a **.psbt** extension.

<br>

<a href="./../../../save_the_transaction.png" target="_blank">
    <img id="sparrow_wallet_multisig_send_save" src="./../../../save_the_transaction.png" alt="sparrow_wallet_multisig_save" width="600"/> 
</a>
 
<br>

**4\.** It's time to sign this transaction using 2 of the 3 Coldcards. Add the **psbt** file to a MicroSD card. Make sure to have you passphrase ready,
        either on the MicroSD card or ready to enter manually.

<br>

**5\.** Insert the MicroSD card into the Coldcard and enter your master PIN.

<br>

**6\.** Select **Passphrase**. If you have it saved to the MicroSD card then select **Restore Saved**, otherwise select **edit** and enter it manually.

<br>

**7\.** The Coldcard will load the passphrase. you can confirm that the correct one was added by confirming that the **Extended Fingerprint** is correct.

<br>

**8\.** Select **Ready to Sign**. The Coldcard will ask which PSBT file you wish to sign, there should only be one at this stage. (When signing with the second Coldcard
        there will be 2, make sure to select the one that ends in  **-part.pbst**) 

<br>

**9\.** The Coldcard will display the amount, destination address and the associated network fee. Make sure to verify that the information displayed
        is correct.

<br> 

**10\.** Press enter or 1(review this for mk4) to approve and sign the transaction. The PSBT will be signed and a new file created with **-part** will be appended to the
         original name.

<br> 

**11\.** Repeat this step on the second Coldcard. Remember to select the PSBT file ending in *-part.pbst**

<br> 

**12\.** Once you've finished signing, insert the MicroSD card back into your computer, go back to Sparrow Wallet and click on **Load Transaction**, select
         the PSBT file, it likely ends in **-part-2.psbt**.

<br> 

**13\.** After loading the PSBT, 2 signatures will appear, one for each Coldcard or signing device. The blue **Broadcast Transaction"" button will become clickable,
         click it to broadcast your transaction. 
         
<br> 

<a href="./../../../signed_ready_to_broadcast.png" target="_blank">
    <img id="sparrow_wallet_multisig_signed_broadcast" src="./../../../signed_ready_to_broadcast.png" alt="sparrow_wallet_multisig_signed_broadcast" width="600"/> 
</a>
 
<br>

**14\.** Sparrow will broadcast your transaction to the network, once complete you'll see a page similar to the one below. 

<br>

<a href="./../../../transaction_sent.png" target="_blank">
    <img id="sparrow_wallet_multisig_transaction_sent" src="./../../../transaction_sent.png" alt="sparrow_wallet_multisig_transaction_sent" width="600"/> 
</a>

<br>

**Congrats, you've sent a bitcoin transaction using multiple co-signers.**
