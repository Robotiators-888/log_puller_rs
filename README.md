# log_puller_rs
Automatically pulls logs from a robo rio (or systemcore)\
Just do cargo run --release and it will try to pull logs every 3 seconds until it is able to find a robot\
Once it finds a robot, it will incrementally back up logs using the file sizes\
It will then wait 60 seconds to pull again\
If it fails to pull, it will go back to the 3 second loop\
Note: pressing control + c will wait for the current log to finish copying before exiting\
Use cargo run --release to run after installing rust from [rustup](https://rustup.rs/)
