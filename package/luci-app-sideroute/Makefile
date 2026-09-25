include $(TOPDIR)/rules.mk

PKG_NAME:=luci-app-sideroute
PKG_VERSION:=1.0.0
PKG_RELEASE:=1

PKG_MAINTAINER:=Mozi <mozi1924@users.noreply.github.com>
PKG_LICENSE:=MIT

include $(INCLUDE_DIR)/package.mk

define Package/luci-app-sideroute
  SECTION:=luci
  CATEGORY:=LuCI
  SUBMENU:=3. Applications
  TITLE:=LuCI Support for Side-Route Daemon (Dual-Stack Failover & Inbound Protection)
  DEPENDS:=+nftables +kmod-nft-core +kmod-nft-nat +ip-full
  PKGARCH:=all
endef

define Package/luci-app-sideroute/description
  A robust IPv4/IPv6 dual-stack side-routing daemon and LuCI controller for OpenWrt (25.x / fw4 / nftables).
  Features automatic health failover, bypass for LAN traffic, and full connection-tracking
  protection for WAN port-forwarded and public IPv6 services.
endef

define Build/Compile
	# Binary can be pre-compiled or built via cargo
	true
endef

define Package/luci-app-sideroute/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) ./root/usr/bin/side-route-daemon $(1)/usr/bin/side-route-daemon

	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_CONF) ./root/etc/config/sideroute $(1)/etc/config/sideroute

	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./root/etc/init.d/sideroute $(1)/etc/init.d/sideroute

	$(INSTALL_DIR) $(1)/usr/share/luci/menu.d
	$(INSTALL_DATA) ./root/usr/share/luci/menu.d/luci-app-sideroute.json $(1)/usr/share/luci/menu.d/luci-app-sideroute.json

	$(INSTALL_DIR) $(1)/usr/share/rpcd/acl.d
	$(INSTALL_DATA) ./root/usr/share/rpcd/acl.d/luci-app-sideroute.json $(1)/usr/share/rpcd/acl.d/luci-app-sideroute.json

	$(INSTALL_DIR) $(1)/www/luci-static/resources/view/sideroute
	$(INSTALL_DATA) ./root/www/luci-static/resources/view/sideroute/overview.js $(1)/www/luci-static/resources/view/sideroute/overview.js
endef

$(eval $(call BuildPackage,luci-app-sideroute))
