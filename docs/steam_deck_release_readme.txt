OBSERVED 2 - STEAM DECK / LINUX
================================

This is a native x86_64 Linux build targeting Steam Linux Runtime 4.

The `observed` executable and `assets` directory must remain beside each other.
From a terminal, launch the game with:

    ./observed

Steam Deck setup
----------------

1. Extract the complete archive in Desktop Mode.
2. In Steam, choose Games > Add a Non-Steam Game to My Library.
3. Browse to and select `observed` from the extracted directory.
4. Leave "Force the use of a specific Steam Play compatibility tool" disabled;
   this is a native Linux executable.
5. Return to Gaming Mode and launch Observed 2.

The archive preserves the executable permission. If another archive program removes
it, restore it from a terminal with:

    chmod +x observed

Advanced asset override
-----------------------

Launchers may set OBSERVED2_ASSET_ROOT to an alternate `assets` directory. Normal
GitHub Release and Steam installs do not need this variable.
