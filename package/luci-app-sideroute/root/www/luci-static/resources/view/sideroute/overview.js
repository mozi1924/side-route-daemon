'use strict';
'require view';
'require form';
'require fs';
'require poll';
'require network';
'require uci';
'require rpc';
'require ui';
'require dom';

var callRcInit = rpc.declare({
	object: 'rc',
	method: 'init',
	params: ['name', 'action'],
	expect: { '': {} }
});

function renderBadge(res) {
	if (!res || !res.running) {
		return E('span', { 'class': 'badge badge-warning' }, _('服务未运行 / 已禁用'));
	} else if (res.online) {
		return E('span', { 'class': 'badge badge-success' }, _('旁路由在线 (已分流)'));
	} else {
		return E('span', { 'class': 'badge badge-danger' }, _('旁路由离线 (已降级直连)'));
	}
}

return view.extend({
	load: function() {
		return Promise.all([
			fs.read_direct('/tmp/sideroute.json', 'json').catch(function() { return null; }),
			network.getHostHints()
		]);
	},

	render: function(data) {
		var status = data[0] || {};
		var hosts = data[1] || {};

		var m, s, o;

		m = new form.Map('sideroute', _('Side-Route Daemon (双栈旁路由分流管理)'),
			_('专为 OpenWrt (fw4/nftables) 设计的高可用旁路由守护程序。支持 IPv4/IPv6 双栈智能分流、局域网直连保护、秒级宕机容灾回退，并彻底解决防火墙端口放行设备的外网回包问题。'));

		// --- 状态卡片展示区 ---
		s = m.section(form.NamedSection, 'global_status', 'status', _('运行状态'));
		s.render = function() {
			var elState = E('div', { 'id': 'sr_state' }, [ renderBadge(status) ]);
			var elV4 = E('code', { 'id': 'sr_ipv4' }, [ status.side_ipv4 || _('未发现 / 未启用') ]);
			var elV6 = E('code', { 'id': 'sr_ipv6' }, [ status.side_ipv6 || _('未发现 / 未启用') ]);
			var elClients = E('span', { 'class': 'badge', 'id': 'sr_clients' }, [
				((status.client_count !== undefined) ? status.client_count : 0) + ' ' + _('台设备')
			]);
			var elMsg = E('span', { 'id': 'sr_msg', 'style': 'color:#666;' }, [ status.message || _('无') ]);

			var statusTable = E('table', { 'class': 'table' }, [
				E('tr', { 'class': 'tr' }, [
					E('td', { 'class': 'td left', 'style': 'width:25%' }, _('当前状态')),
					E('td', { 'class': 'td left' }, [ elState ])
				]),
				E('tr', { 'class': 'tr' }, [
					E('td', { 'class': 'td left' }, _('旁路由 IPv4')),
					E('td', { 'class': 'td left' }, [ elV4 ])
				]),
				E('tr', { 'class': 'tr' }, [
					E('td', { 'class': 'td left' }, _('旁路由 IPv6')),
					E('td', { 'class': 'td left' }, [ elV6 ])
				]),
				E('tr', { 'class': 'tr' }, [
					E('td', { 'class': 'td left' }, _('受保护代理设备数')),
					E('td', { 'class': 'td left' }, [ elClients ])
				]),
				E('tr', { 'class': 'tr' }, [
					E('td', { 'class': 'td left' }, _('状态信息')),
					E('td', { 'class': 'td left' }, [ elMsg ])
				])
			]);

			var btnBar = E('div', { 'style': 'margin-top: 12px; display: flex; gap: 8px;' }, [
				E('button', {
					'class': 'btn cbi-button cbi-button-action',
					'click': ui.createHandlerFn(this, function() {
						return callRcInit('sideroute', 'enable').then(function() {
							return callRcInit('sideroute', 'start');
						}).then(function() {
							ui.addNotification(null, E('p', {}, _('Side-Route 守护进程启动请求已发送。')), 'info');
						}).catch(function(e) {
							ui.addNotification(null, E('p', {}, _('启动失败: %s').format(e.message || e)), 'error');
						});
					})
				}, _('▶ 启动服务')),
				E('button', {
					'class': 'btn cbi-button cbi-button-action',
					'click': ui.createHandlerFn(this, function() {
						return callRcInit('sideroute', 'enable').then(function() {
							return callRcInit('sideroute', 'restart');
						}).then(function() {
							ui.addNotification(null, E('p', {}, _('Side-Route 守护进程重启请求已发送。')), 'info');
						}).catch(function(e) {
							ui.addNotification(null, E('p', {}, _('重启失败: %s').format(e.message || e)), 'error');
						});
					})
				}, _('🔄 重启服务')),
				E('button', {
					'class': 'btn cbi-button cbi-button-negative',
					'click': ui.createHandlerFn(this, function() {
						return callRcInit('sideroute', 'stop').then(function() {
							ui.addNotification(null, E('p', {}, _('Side-Route 守护进程已停止。')), 'info');
						}).catch(function(e) {
							ui.addNotification(null, E('p', {}, _('停止失败: %s').format(e.message || e)), 'error');
						});
					})
				}, _('⏹ 停止服务'))
			]);

			// 注册定期轮询更新
			poll.add(function() {
				return fs.read_direct('/tmp/sideroute.json', 'json').then(function(res) {
					var sNode = document.getElementById('sr_state');
					var v4Node = document.getElementById('sr_ipv4');
					var v6Node = document.getElementById('sr_ipv6');
					var cNode = document.getElementById('sr_clients');
					var mNode = document.getElementById('sr_msg');

					if (!sNode) return;

					dom.content(sNode, renderBadge(res));
					if (v4Node) v4Node.textContent = (res && res.side_ipv4) ? res.side_ipv4 : _('未发现 / 未启用');
					if (v6Node) v6Node.textContent = (res && res.side_ipv6) ? res.side_ipv6 : _('未发现 / 未启用');
					if (cNode) cNode.textContent = ((res && res.client_count !== undefined) ? res.client_count : 0) + ' ' + _('台设备');
					if (mNode) mNode.textContent = (res && res.message) ? res.message : _('无');
				}).catch(function() {});
			}, 3);

			return E('div', { 'class': 'cbi-section' }, [
				E('h3', {}, _('运行状态')),
				E('div', { 'class': 'cbi-section-node', 'style': 'padding: 10px;' }, [
					statusTable,
					btnBar
				])
			]);
		};

		// --- 核心配置区 ---
		s = m.section(form.TypedSection, 'sideroute', _('基本设置'));
		s.anonymous = true;
		s.addremove = false;

		o = s.option(form.Flag, 'enabled', _('启用守护进程'), _('启用后系统将自动探测旁路由并管理分流策略路由与防火墙规则。'));
		o.rmempty = false;
		o.default = '0';

		// 旁路由 MAC 地址
		o = s.option(form.Value, 'side_mac', _('旁路由 MAC 地址'),
			_('指定旁路由的物理网卡 MAC 地址。系统将根据该 MAC 自动在局域网内解析其 IPv4 与 IPv6。'));
		o.rmempty = false;
		o.datatype = 'macaddr';
		o.placeholder = 'bc:24:11:xx:xx:xx';

		// 从 hosthints 填充已知设备作为建议选项
		if (hosts && typeof hosts.getHostsByMAC === 'function') {
			var macMap = hosts.getHostsByMAC();
			for (var mac in macMap) {
				var info = macMap[mac];
				var name = info.name || info.ipv4 || info.ipv6 || '';
				var label = mac.toUpperCase() + (name ? ' (' + name + ')' : '');
				o.value(mac.toLowerCase(), label);
			}
		}

		// 旁路由固定 IPv4（可选）
		o = s.option(form.Value, 'side_ipv4', _('旁路由固定 IPv4 (可选)'),
			_('可留空。留空时系统将通过 ARP 自动从 MAC 地址解析旁路由的 IPv4 地址；若填写则优先固定该 IP。'));
		o.datatype = 'ip4addr';
		o.placeholder = '192.168.1.2';

		// 协议开关
		o = s.option(form.Flag, 'enable_ipv4', _('开启 IPv4 旁路分流'), _('将所选设备的 IPv4 外网流量转发到旁路由。'));
		o.default = '1';

		o = s.option(form.Flag, 'enable_ipv6', _('开启 IPv6 旁路分流'), _('将所选设备的 IPv6 外网流量转发到旁路由。'));
		o.default = '1';

		o = s.option(form.Flag, 'hijack_dns', _('劫持 DNS 请求 (端口 53)'),
			_('将所选设备的 53 端口 DNS 请求重定向至旁路由处理。'));
		o.default = '1';

		// --- 分流设备管理 ---
		s = m.section(form.TypedSection, 'sideroute', _('分流代理设备列表'));
		s.anonymous = true;
		s.addremove = false;

		o = s.option(form.DynamicList, 'client_mac', _('指定代理设备的 MAC 地址'),
			_('在此处添加需要走旁路由代理的局域网设备。列表中所选设备的公网出站流量将由旁路由接管；<strong>即使在此列表中的设备，其外网端口转发（DNAT）与公网 IPv6 入站访问依然享受主路由防火墙连接保护，回包直出 WAN 口，绝不掉线。</strong>'));
		o.datatype = 'macaddr';
		o.placeholder = '00:11:22:33:44:55';

		if (hosts && typeof hosts.getHostsByMAC === 'function') {
			var macMap = hosts.getHostsByMAC();
			for (var mac in macMap) {
				var info = macMap[mac];
				var name = info.name || info.ipv4 || info.ipv6 || '';
				var label = mac.toUpperCase() + (name ? ' (' + name + ')' : '');
				o.value(mac.toLowerCase(), label);
			}
		}

		// --- 高级设置 ---
		s = m.section(form.TypedSection, 'sideroute', _('高级网络与接口参数'));
		s.anonymous = true;
		s.addremove = false;

		o = s.option(form.Value, 'wan_interface', _('WAN 接口名称'),
			_('主路由外网接口设备名（如 pppoe-wan, wan 等），用于连接跟踪入站保护打标。'));
		o.default = 'pppoe-wan';

		o = s.option(form.Value, 'lan_interface', _('LAN 接口名称'),
			_('主路由内网接口设备名（如 br-lan），用于探测旁路由及路由出接口绑定。'));
		o.default = 'br-lan';

		o = s.option(form.Value, 'check_interval', _('存活探测间隔 (秒)'),
			_('定时探测旁路由心跳的时间周期（秒），旁路由宕机时系统将在 1 个探测周期内透明回退主路由直连。'));
		o.datatype = 'range(1, 60)';
		o.default = '3';

		o = s.option(form.Value, 'table_id', _('策略路由表 ID'), _('专用的 Linux 策略路由表编号，默认 200。'));
		o.datatype = 'range(1, 252)';
		o.default = '200';

		o = s.option(form.Value, 'mark_id', _('防火墙连接标记 (fwmark)'), _('分流匹配用的 fwmark ID，默认 1。'));
		o.datatype = 'range(1, 65535)';
		o.default = '1';

		return m.render();
	},

	handleSaveApply: function(ev, mode) {
		return this.handleSave(ev).then(L.bind(function() {
			return ui.changes.apply(mode == '0').then(L.bind(function() {
				return uci.load('sideroute').then(function() {
					var isEnabled = uci.get('sideroute', 'config', 'enabled') === '1';
					if (isEnabled) {
						return callRcInit('sideroute', 'enable').then(function() {
							return callRcInit('sideroute', 'restart');
						});
					} else {
						return callRcInit('sideroute', 'stop');
					}
				});
			}, this));
		}, this));
	}
});
