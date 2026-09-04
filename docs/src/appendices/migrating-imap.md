# Kopieren der E-Mail-Konten von einem IMAP-Server zu einem anderen mit imapsync

To migrate a single email account from one IMAP server to another, you can use the `imapsync` tool. Below is an example command that demonstrates how to perform this migration:

```bash
imapsync \
  --host1 solawis.de \
  --user1 webportal@solawis.de \
  --password1 'pass' \
  --host2 mx.solawis.de --ssl2 \
  --user2 webportal@solawis.de \
  --password2 'pass' \
  --syncinternaldates --automap --nosslcheck
```

If you want to migrate multiple accounts, you can create a CSV file named `imapsync-text.csv` with the following format:

```
host1|user1|password1|host2|user2|password2||
```

Then create a new file called `imapsync-skript.sh` and add the following content:

```bash
#!/bin/sh

echo "Looping on accounts credentials found imapsync-text.csv"
echo

line_counter=0
# Leert die Fehler-Logdatei zu Beginn
> file_failures.txt

{ while IFS='|' read h1 u1 p1 h2 u2 p2 extra fake
    do
        line_counter=`expr 1 + $line_counter`

        # Überspringt Zeilen, die mit # beginnen oder komplett leer sind
        { echo "$h1" | tr -d '\r' | egrep '^#|^ *$' ; } > /dev/null && continue

        echo "==== Starting imapsync with --host1 $h1 --user1 $u1 --host2 $h2 --user2 $u2 $extra $@ ===="
        echo "Got those values from file.txt presented inside brackets: [$h1] [$u1] [$h2] [$u2] [$extra] [$fake]"

        # Aufruf ohne 'eval', um Sonderzeichen in Passwörtern zu schützen.
        # --nosslcheck wurde durch --nosslcheck1 und --nosslcheck2 ersetzt.
        if imapsync --host1 "$h1" --user1 "$u1" --password1 "$p1" \
                    --host2 "$h2" --ssl2 --user2 "$u2" --password2 "$p2" \
                    --syncinternaldates --automap --nosslcheck $extra "$@"
        then
                echo "success sync for line $line_counter "
        else
                echo "$h1;$u1;$h2;$u2;$extra;" | tee -a file_failures.txt
        fi
        echo "==== Ended imapsync with --host1 $h1 --user1 $u1 --host2 $h2 --user2 $u2 $extra $@ ===="
        echo
    done
} < imapsync-text.csv
```

The script reads the credentials from the CSV file and executes the `imapsync` command for each account. It also logs any failures to a file named `file_failures.txt`.

After successfully migrating add a line to the original servers `/etc/postfix/migration_transport` file to redirect the email traffic to the new server.

```
# Example entry in /etc/postfix/migration_transport
dmarc@solawis.de      smtp:[mx.solawis.de]:587
```

After that you need to reload the Postfix configuration to apply the changes:

```bash
# postmap /etc/postfix/migration_transport
# systemctl reload postfix
```
