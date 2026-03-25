// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

interface IKMSVerifiable {
    function oysterKMSVerify(bytes32 imageId) external view returns (bool);
}

contract LightfriendKmsVerifiable is IKMSVerifiable {
    error NotOwner();
    error PendingActivation();
    error NothingPending();
    error TimelockActive(uint256 activatesAt);

    struct PendingImage {
        bytes32 imageId;
        uint256 activatesAt;
        string commitHash;
    }

    address public owner;
    uint256 public cooldownSeconds;
    PendingImage public pendingApproval;
    mapping(bytes32 => bool) public approvedImages;

    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event CooldownUpdated(uint256 previousCooldownSeconds, uint256 newCooldownSeconds);
    event ImageProposed(bytes32 indexed imageId, string commitHash, uint256 activatesAt);
    event PendingImageCleared(bytes32 indexed imageId);
    event ImageActivated(bytes32 indexed imageId);
    event ImageRevoked(bytes32 indexed imageId);

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    constructor(uint256 initialCooldownSeconds) {
        owner = msg.sender;
        cooldownSeconds = initialCooldownSeconds;
        emit OwnershipTransferred(address(0), msg.sender);
        emit CooldownUpdated(0, initialCooldownSeconds);
    }

    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "new owner is zero");
        emit OwnershipTransferred(owner, newOwner);
        owner = newOwner;
    }

    function setCooldownSeconds(uint256 newCooldownSeconds) external onlyOwner {
        emit CooldownUpdated(cooldownSeconds, newCooldownSeconds);
        cooldownSeconds = newCooldownSeconds;
    }

    function proposeImage(bytes32 imageId, string calldata commitHash) external onlyOwner {
        if (pendingApproval.imageId != bytes32(0)) revert PendingActivation();

        uint256 activatesAt = block.timestamp + cooldownSeconds;
        pendingApproval = PendingImage({
            imageId: imageId,
            activatesAt: activatesAt,
            commitHash: commitHash
        });

        emit ImageProposed(imageId, commitHash, activatesAt);
    }

    function activateImage() external {
        PendingImage memory pending = pendingApproval;
        if (pending.imageId == bytes32(0)) revert NothingPending();
        if (block.timestamp < pending.activatesAt) revert TimelockActive(pending.activatesAt);

        approvedImages[pending.imageId] = true;
        delete pendingApproval;

        emit ImageActivated(pending.imageId);
        emit PendingImageCleared(pending.imageId);
    }

    function clearPendingImage() external onlyOwner {
        bytes32 imageId = pendingApproval.imageId;
        if (imageId == bytes32(0)) revert NothingPending();
        delete pendingApproval;
        emit PendingImageCleared(imageId);
    }

    function revokeImage(bytes32 imageId) external onlyOwner {
        delete approvedImages[imageId];

        if (pendingApproval.imageId == imageId) {
            delete pendingApproval;
            emit PendingImageCleared(imageId);
        }

        emit ImageRevoked(imageId);
    }

    function oysterKMSVerify(bytes32 imageId) external view override returns (bool) {
        return approvedImages[imageId];
    }
}
