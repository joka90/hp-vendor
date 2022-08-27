# hp-vendor

This project provides HP-specific plugins for Pop!\_OS.

# Build

	dpkg-buildpackage -us -uc
	sudo dpkg -i ../pop-hp-vendor_*_amd64.deb ../pop-hp-vendor-dkms_*_amd64.deb

# To enable unsupported hardware
Modify configuration

	echo 'allow_unsupported_hardware = true' | sudo tee /etc/hp-vendor.conf

and add DMI data to DKMS driver `dkms/hp_vendor.c`

	sed -i "0,/	{}/s|	{}|	{\n		.ident = \"$(cat /sys/class/dmi/id/product_name)\",\n		.matches = {\n			DMI_MATCH(DMI_BOARD_VENDOR, \"HP\"),\n			DMI_MATCH(DMI_BOARD_NAME, \"$(cat /sys/class/dmi/id/board_name)\"),\n		},\n	},\n&|" dkms/hp_vendor.c
