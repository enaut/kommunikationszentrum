# Kopieren der E-Mail-Konten von einem IMAP-Server zu einem anderen mit imapsync

imapsync \
  --host1 solawis.de \
  --user1 webportal@solawis.de \
  --password1 'pass' \
  --host2 mx.solawis.de --ssl2 \
  --user2 webportal@solawis.de \
  --password2 'pass' \
  --syncinternaldates --automap --nosslcheck
