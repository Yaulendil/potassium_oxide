# Auctions

**NOTE: This guide assumes that you are using the default value for the `prefix` setting.**


## Before an Auction

This bot sends a large number of messages in a very short period of time while an auction runs. Twitch does not like it very much when a normal user does this. As a result, it is highly recommended to make sure that the account that will be used to run auctions is set to be a channel VIP. This should help to ensure that the bot is able to post important auction-related information, including winners, in the chat.

Being a VIP will also allow the bot to post in channels with chat restrictions in place, such as subscriber-only mode.


## Running an Auction

The basic command to run an Auction is `+auction start`. The bot will send a long message explaining how to bid, and immediately start an Auction using the default values specified in the Configuration file. You can view all the default Configuration values ahead of time with the `+config` command.

Additional options can be specified with dashed "flag" syntax; the short form of an option can be specified with one dash, like `-o value`, while the longer full-word form can be specified with two dashes, like `--option value`. There is no difference between how the short and long forms work, and you can use whichever version you prefer.

The following options can be used to override the values in the configuration file:

- `-t` / `--time`: This changes **how long** the Auction will run. The value should be a positive whole number of seconds. For example, either `+auction start --time 240` or `+auction start -t 240` will start an Auction that runs for 240 seconds (4 minutes).

- `-h` / `--helmet`: This changes the **Helmet¹ value**. For example, `+auction start --helmet 60` will start an Auction with a 60-second Helmet. If this is set to 0, there will be no protection against snipers.

- `-r` / `--raise`: This changes the **raise limit**. For example, if you start an Auction with `+auction start --raise 10`, and someone bids $30, the next person will not be permitted to bid more than $40. This is helpful to stop bids from quickly climbing out of control, or to stop trolls from submitting absurdly high bids and forcing a rerun.

- `-m` / `--min`: This changes the **minimum bid**. For example, if you start an Auction with `+auction start --min 10`, the first bid of the Auction may not be lower than $10.

- `--prize`: This option takes a text value, and will cause the Auction to be described by the bot as "an Auction for (description)", instead of simply "an Auction". For example, `+auction start --prize "a very cool hat"` will start an Auction like normal, but the bot will always mention that there is a very cool hat available when it posts updates about the Auction. See the section on Prizes at the bottom of the page for more information about where else this is used. **IMPORTANT:** If the Prize phrase has multiple words, **the whole phrase MUST be enclosed in quotation marks.** You may use either 'single quotes' or "double quotes", but be aware that 'single quotes' might be parsed incorrectly if there is an apostrophe in the phrase.

Options and their different forms may be mixed freely, and may be in any order. For example, `+auction start -t 120 --helmet 20 -m 15` will start an Auction which **lasts for 120 seconds**, has a **Helmet¹ of 20 seconds**, and has a **minimum bid of $15**.

If an option is given multiple times, its **last specified value** will be used. For example, `+auction start -t 60 -t 120` will start an Auction which lasts **120 seconds**.

---

¹ Helmets protect against snipers. When someone submits a bid, if the remaining time is less than the Helmet value, **the Helmet value will be added to the timer.**


## Modifying an Auction

There are a few commands available to interact with an Auction while it is running:

- `+auction status`: This will cause the bot to reply to you, telling you what the Prize is (if any), how long is left, and the current top bidder. Anyone may use this command.

- `+auction stop`: This will immediately stop the currently active Auction. No winner will be declared, and no Summary file will be saved. All bids will be thrown away.

- `+auction prize`: This will change the value of the Prize for the active Auction. The new value may be put in quotation marks like the `--prize` option described above, but it **does not _need_** to be quoted. This is because this command does not need to look for anything else that may come after the new Prize, so it is able to take everything you type as part of the new Prize. For example, `+auction prize a very cool hat` will change the Prize to "a very cool hat", and going forward, the Auction will act as though that had been the Prize from the very beginning. This allows you to specify a Prize which has both "double quotes" and apostrophes in it.


## Prizes

During the course of an Auction, the bot will periodically post reminders that it is running, as well as the value of the current bid. If you specify a Prize, it will also be included in these reminders. When the Auction ends, the final message declaring the winner will then also declare what the winner has won.

After an Auction finishes, a summary file in [TOML](https://en.wikipedia.org/wiki/TOML) format will be saved to disk detailing the settings it used, as well as who won, and how much they bid. If a Prize is specified, it will be included in the summary file on the top line, and it will also be mentioned in the **name** of the file, to make it very easy to find later.
