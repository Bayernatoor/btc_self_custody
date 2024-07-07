### Setting up the Coldcard

<h4 class="text-2xl pb-2 text-[#f7931a] font-semibold">Setting up Your Coldcard</h4>

You've gathered all the parts, now is the time to setup your Coldcard. There are many guides and approaches to setting up a Coldcard, from very simple to complex.
In this guide we'll provide a balanced approach to ensure privacy and security without overwhelming you with technical details.

<br>

This guide has several steps but the process itself is relatively simple. Take it slow and you'll be setup in no time. If you have any questions throughout the process feel free to reachout to me via 
<a class="text-blue-400 hover:text-[#3c6594]" href="mailto:wehodlbtc@pm.me" target="_blank" rel="noopener noreferrer">email</a> or through the links in the footer.


<br>

<h5 class="text-xl pb-4 text-[#f7931a] font-semibold">Visual Inspection</h5>

**1\.** Upon receiving your Coldcard, inspect the tamper evident bag, ensure it wasn't opened or tampered with. 

<br>

**2\.** Open the bag and verify that the device has not been tampered with.
 
<br>

**3\.** The bag itself has a unique serial number on it. We'll match that number to the one on the Coldcard, so don't dispose of it yet.

<br>

**4\.** Inside the bag you'll find a Coldcard, a serialized tear off tab of the bag and a Wallet Backup Card. The number on the tab should match the number on the bag.

<br>

<h5 class="text-xl pb-2 text-[#f7931a] font-semibold">Updating the Firmware</h5>


**5\.** Download the Coldcard's latest <a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://coldcard.com/docs/upgrade/" target="_blank" rel="noopener noreferrer">firmware here</a>.

<br>

**6\.** After downloading the firmware to your computer, I highly recommend that you
<a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://coldcard.com/docs/upgrade/#dont-trust-verify-the-firmware" target="_blank" rel="noopener noreferrer">verify it by following this guide</a>.

<br>

**7\.** Once downloaded & verified, connect one of the MicroSD cards to your computer and copy the firmware file to it. 

<br>

**8\.** Connect your Coldcard to power **(DO NOT connect it to a computer)**. Use Coldpower or a USB battery/wall charger.

<br>

***Note:*** *Some battery packs will turn off when connected to low power devices I recommend using a USB wall charger or coldpower.*

<br>

**9\.** Your Coldcard will power up, verify that the serial number on the device matches the one on the bag.

<br>

**10\.** If it matches, press the **Checkmark** in the bottom right corner. 

<br>

**11\.** Next, insert the MicroSD card into the Coldcard. Click on ***Advanced -> Upgrade Firmware -> From MicroSD***. Select the firmware to use, wait for the Coldcard to update.

<br>

<h5 class="text-xl pb-2 text-[#f7931a] font-semibold">Setting a Pin</h5>

**12\.** The PIN grants complete access to your Coldcard so choose it wisely. It consists of a prefix and a suffix each comprised of 2-6 digits.
        **There is no way to recover this PIN so keep it safe!**

<br>

**13\.** Select **Choose Pin Code**.

<br>

**14\.** Enter the prefix (first part of the pin), I recommend using at least 4 digits. Write it down on the included backup card.

<br>

**15\.** After pressing the **Checkmark** you'll be presented with **2 anti-phishing words**. Make note of these on the backup card. These words
        will appear each time you enter your prefix. They confirm that your Coldcard has not been tampered with since you last accessed it.

<br>

**16\.** Now enter the suffix, again use a minimum of 4-6 digits and write them down on the backup card.

<br>

**17\.** You'll be asked to re-enter the prefix & suffix and confirm the anti-phising words. Make sure you wrote everything down correctly.

<br>

<h5 class="text-xl pb-2 text-[#f7931a] font-semibold">Creating a New Wallet</h5>

**NOTE: The Coldcard came with 2 MicroSD cards. Use one of the 2 for the encrypted wallet backup that you'll generate after wallet creation. Use the second to save an encrypted copy of your Passphrase and for 
        signing transactions.**

<br>

**18\.** From the main page, press on **New Wallet**. After a moment 24 words will appear.

<br>

**19\.** So as to not fully trust Coldcard's random number generator, we'll add our own entropy and we'll do so by rolling some dice.

<br>

**20\.** Press 4 to add dice rolls to your seed.

<br>

**21\.** Roll a **minimum** of 100 dice, adding each roll to the Coldcard, when finished press the **Checkmark**. Don't cheat, make sure you actually roll the dice and enter the number, otherwise your security may be weaken.

<br>

**22\.** A new list of 24 words will be displayed. Copy these words to the backup card. Take your time and make sure you copy them correctly.

<br>

**23\.** Time for a test. Coldcard will ask you to confirm all the words in an arbitrary order.  

<br>

**24\.** Congrats, you've successfully created a new seed (private key) on your Coldcard. But we're not done yet. 

<br>

**25\.** If you're using this device to protect large sums of bitcoin you'll want to make sure you did everything correctly. To confirm that you did, you'll delete the seed from the device
and restore it using the words you wrote down. 

<br>

**26\.** Every seed generates a unique Fingerprint (AKA Extended/Master Fingerprint), let's write that down. Click on ***Advanced -> View Identity***. A unique Fingerprint will appear, write it down.

<br>

**27\.** Let's delete the current seed on your Coldcard. Go to ***Advanced -> Danger Zone -> Seed Functions -> Destroy Seed***. Read and agree to the warnings.

<br>

**28\.** Re-enter your pin to access your Coldcard. Go to ***Import Existing -> 24 words***. Re-enter your seed. Use the arrows to scroll down to select the first letter, 
second letter and so on, repeat the process for each word. Once you get to the 23rd word, Coldcard with present you with several options for the 24th word, select the correct one. If your 24th word
does not appear you either made a mistake or incorrectly copied the words (try re-entering the words). 

<br>

**29\.** Once you've entered all 24 words press on the **Checkmark** to confirm. Go to ***Advanced -> View Identity*** and confirm that you've actually restored the original seed words
         by verifying that the **Extended Fingerprint** matches. 

<br>

<h5 class="text-xl pb-4 text-[#f7931a] font-semibold">Adding a Passphrass</h5>

**30\.** A Passphrase acts as **"25th word"** and helps add additional security to your seed words. Adding a Passphrase would help prevent access to your wallet if someone 
was able to obtain your 24 words. Remember, the original 24 words result in a valid wallet, adding a 25th word creates an entirely new wallet.

<br>

**31\.** Adding a Passphrase to your 24 seed words will result in a new Extended Fingerprint. It's important to write this down since the Coldcard cannot confirm if you've entered
the correct Passphrase. But you can confirm it via the unique Extended Fingerprint.

<br>

**32\.** Let's begin. Select **Passphrase** from the main menu. 

<br>

**33\.** Read the warnings and press the **Checkmark**. 

<br>

**34\.** I recommend selecting a combination of words, numbers and special characters, make sure it's at least 12 chars in length.

<br>

**35\.** Write this Passphrase down and/or stamp it onto steel, then store it in a safe place. Also, **don't store it with your seed words**. 
         But keep it safe since, **it's just as important as your 24 seed words.**

<br>

**36\.** Copy the **XFP (Extended/Master Fingerprint)** down as well. If you ever restore your wallet, you'll use the Fingerprint to confirm that you've entered the correct seed words and Passphrase. 

<br>

**37\.** When satisfied, press on **APPLY**. Ensure you have the second MicroSD card inserted and press 1 to create & save an encrypted backup of the Passphrase. 
         The previous encrypted backup you made of the seed words does NOT save your Passphrase. Make sure that you've also written the Passphrase down and keep it safe! 
         For more information on the encrypted backups created by Coldcard see the 
         <a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://coldcard.com/docs/backups/#background" target="_blank" rel="noopener noreferrer">ColdCard Docs</a>.

<br>

**38\.**  Your Coldcard is now using a new wallet comprised of the 24 original words plus your Passphrase. 

<br>

***Note:*** *The ColdCard does not remember your Passphrase, you must enter it everytime you turn on the device. To do so enter your PIN, insert the MicroSD card and select ***Passphrase --> Restore Saved***. 
        Select the correct Passphrase, after loading the Passphrase will be in effect until you logout. 
        For more detailed information on Passphrases see the*
        <a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://coldcard.com/docs/passphrase/#using-a-saved-passphrase" target="_blank" rel="noopener noreferrer">ColdCard Docs</a>.

<br>

<h5 class="text-xl pb-2 text-[#f7931a] font-semibold">Summary</h5>

Congrats, you've successfully set up your Coldcard with a 24 word seed and a Passphrase. at this point you should have the following:
a backup card with your **pin suffix + prefix** as well as your **2 anti-phishings** words, your **24 seed words** and that seed's **Extended Fingerprint**. 
You should also have an encrypted backup of your **Seed Words**, your **Passphrase** and the new **Master Fingerprint**. 

<br>

***Note:*** *Your Coldcard stores your 24 seed words but not your Passphrase, everytime you access your wallet you'll need to enter your Passphrase.*
*after adding the Passphrase double check that it's the correct wallet by confirming that the Extended Fingerprint matches.*

<br>

- For more details on how Passphrases work click
<a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://coldcard.com/docs/passphrase/#passphrases-and-your-coldcard" target="_blank" rel="noopener noreferrer">here</a>.

<br>

- To view Coldcard's official documention click 
<a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://coldcard.com/docs/" target="_blank" rel="noopener noreferrer">here</a>.

<br>

- To learn about some of Coldcard's best features checkout Econoalchemist's great blog post
<a class="text-[#8cb4ff] underline-offset-auto font-semibold" href="https://www.econoalchemist.com/post/my-top-10-coldcard-features" target="_blank" rel="noopener noreferrer">here</a>.

