[Setting up the Coldcard](#setting-up-the-coldcard)

You've gathered all the parts, now is the time to setup your Coldcard. There are many guides and approaches, some of which I will provide below. However,
this guide will provide a balanced approach to setting up your Coldcard.

&nbsp;

**Visual inspection:**

**1\.** Upon receiving your Coldcard, inspect the tamper evident bag, ensuring it wasn't opened or tampered with. 

**2\.** Open the bag and verify that the device has not been tampered with.
 
**3\.** The bag itself has a unique serial number on it. We'll match that number to the Coldcard, so don't dispose of it yet.

**4\.** Inside the bad you'll find a Coldcard, a serialized tear off tab of the bag and a Wallet Backup Card. The tab should match the bag.

&nbsp;

**Updating the Firmware:**

**5\.** Download the Coldcard's latest **[firmware here](https://coldcard.com/docs/upgrade/)**.

**6\.** After having downloaded the firmware to your computer, I highly recommend that you **[verify it by following this guide](https://coldcard.com/docs/upgrade/#dont-trust-verify-the-firmware)**.

**7\.** Once downloaded/verified, connect one of the MicroSD cards to your computer and copy the firmware file to it. 

**8\.** Connect your Coldcard to power **(DO NOT connect it to a computer)**. Use Coldpower or a USB battery.
*Note: Some battery packs will turn off when connected to low power devices I recommend using a USB wall charger or coldpower.*

**9\.** Your Coldcard will power up, verify that the serial number on the device matches the one on the bag.

**10\.** If it matches, press the **Checkmark** in the bottom right corner. 

**11\.** Next, click on *Advanced* -> *Upgrade Firmware* -> *From MicroSD*. Select the Firmware to use, wait for the Coldcard to update.

&nbsp;

**Setting a Pin:**

**12\.** The PIN grants complete access to your Coldcard so choose it wisely. It consists of a prefix and a sufix each comprised of 2-6 digits.
        **There is no way to recover this PIN so keep it safe!**

**13\.** Select **Choose Pin Code**.

**14\.** Enter the prefix (first part of the pin), I recommend using at least 4 digits. Write it down on the included backup card.

**15\.** After pressing the **Checkmark** you'll be presented with **2 anti-phishing words**. Make note of these on the backup card. These words
        will appear each time you enter your prefix. They confirm that your Coldcard has not been tampered with since you last accessed it.

**16\.** Now enter the suffix, again use a minimum of 4-6 digits and write them down on the backup card.

**17\.** You'll be asked to re-enter the prefix/suffix and confirm the anti-phising words. Make sure you wrote everything down correctly.

&nbsp;

**Creating a new wallet:**

**18\.** Press on **New Wallet**. After a moment 24 words will appear.

**19\.** So as to not fully trust Coldcard's random number generator, we'll add our own entropy and we'll do so by rolling some dice.

**20\.** Press 4 to add dice rolls to your seed.

**21\.** Roll a **minimum** of 100 dice, adding each roll to the Coldcard, when finished press the **Checkmark**.

**22\.** A new list of 24 words will be displayed. Copy these words to the backup card. Take your time and make sure you copy them correctly.

**23\.** Time for a test. Coldcard will ask you to confirm all the words in an arbitrary order.  

**24\.** Congrats, you've successfully created a new seed on your coldcard. But we're not done yet. 

**25\.** If we're using this device to protect large sums of bitcoin we'll want to make sure we did everything correctly. To confirm that we did, we'll delete the seed from the device
and restore it using the words we wrote down. 

**26\.** Every seed generates a unique Fingerprint, let's write that down. Click on *Advanced* -> *View Identity*. A unique Fingerprint will appear, write it down. You can add it to the backup card.

**27\.** Let's delete the current seed on your Coldcard. Go to *Advanced* -> *Danger Zone* -> *Seed Functions* -> *Destroy Seed*. Read and agree to the warnings.

**28\.** Re-enter your pin to access your Coldcard. Go to *Import Exisiting* -> *24 words*. Re-enter your seed. Use the arrows to scroll down to select the first letter, 
second letter and so on, repeat the process for each word. Once you get to the 23rd word, Coldcard with present you with several options for the 24th word, select the correct one. If your 24th word
does not appear you either made a mistake or incorrectly copied the words. 

**29\.** Once you've entered all 24 words press on the **Checkmark** to confirm. Go to *Advanced* -> *View Identity* to confirm that you've in fact restored the original seed. 

<br>

**Adding a Passphrase:**

<br>

**30\.** A Passphrase acts a **"25th word"** and helps add additional security to your seed words. Adding a Passphrase would help prevent access to your wallet if someone 
were able to obtain your 24 words. Remember, the original 24 words results in a valid wallet, adding a 25th word creates an entirely new wallet.  

<br>

**31\.** Adding a Passphrase to your 24 seed words will result in a new Master Fingerprint. It's important to wrote this down since the Coldcard cannot confirm if you've entered
the correct Passphrase. But you can confirm it via the unique Master Fingerprint.

<br>

**32\.** Let's begin. Select **Passphrase** from the main menu. 

<br>

**33\.** Read the warnings and press the **Checkmark**. 

<br>

**34\.** I recommend selecting a combination of words, numbers and special characters, make sure it's at least 12 chars in length.

<br>

**35\.** Write this Passphrase in a safe place. **It's just as important as your 24 seed words.**

<br>

**36\.** Copy the XFP (Extended/Master Fingerprint) down as well. If you ever restore your wallet, you'll use the fingerprint to confirm that you've entered the correct seed and Passphrase. 

<br>

**37\.** When satisfied, press on **APPLY**. Ensure you have a MicroSD card loaded and press 1 to create an encrypted backup of the Passphrase on the MicroSD card. The encrypted backup does NOT save your passphrase.
         Make sure you've written it down and keep it safe! For more information on the encrypted backups created by Coldcard see the **[ColdCard Docs](https://coldcard.com/docs/backups/#background)**.

<br>

**38\.**  Your Coldcard is now using a new wallet comprised of the 24 original words plus your Passphrase. 

<br>

**Note:** *The ColdCard does not remember your passphrase, you must enter it everytime you turn on the device. To do so enter your PIN then select ***Passphrase --> Restore Saved***. 
        The Passphrase is now in effect until you logout. For more detailed information on passphrases see the* **[ColdCard Docs](https://coldcard.com/docs/passphrase/#using-a-saved-passphrase)**


<br>

**Summary:**

&nbsp;

Congrats, you've successfully set up your coldcard with a 24 word seed and a Passphrase. at this point you should have the following:
a backup card with your **pin suffix + prefix** as well as your **2 anti-phishings** words, your **24 seed words** and that seed's **master fingerprint**. 
You should also have an encrypted backup of your **Passphrase** and the new **Master Fingerprint**. 

&nbsp;

*Note: your Coldcard stores your 24 word seed but not your Passphrase, everytime you access your wallet you'll need to enter your Passphrase. you double check that it's the correct wallet by confirming that master fingerpint matches*

&nbsp;

- For more details on how Passphrases work click **[here](https://coldcard.com/docs/passphrase/#passphrases-and-your-coldcard)**.

- To view Coldcard's official documention click **[here](https://coldcard.com/docs/)**.

- To learn about some of Coldcard's best features checkout Econoalchemist's great blog post **[here](https://www.econoalchemist.com/post/my-top-10-coldcard-features)**.
